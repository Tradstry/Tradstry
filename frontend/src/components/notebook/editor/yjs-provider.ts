import * as Y from "yjs";
import type { Provider } from "@lexical/yjs";
import { fromBase64, pendingQueue, toBase64 } from "@tradstry/notebook-core";
import type { GraphQLFetcher } from "@/lib/client";
import {
  appendNotebookUpdates,
  fetchNotebookUpdates,
} from "@/lib/service/notebook-crdt";

type Listener = (...args: unknown[]) => void;

/** Marks updates that arrived from the server, so we never append them back. */
const REMOTE_ORIGIN = Symbol("remote");

const POLL_INTERVAL_MS = 3000;
const FLUSH_INTERVAL_MS = 800;

/**
 * `seq` is a global BIGSERIAL, so a row can become visible out of order: a
 * transaction holding seq 5 may commit after seq 6 is already readable. A cursor
 * of `max(seq)` would skip seq 5 permanently. Re-reading a small window behind
 * the cursor closes that gap; re-applying an update Yjs already has is a no-op.
 */
const REPLAY_WINDOW = 64;

const AWARENESS_STUB = {
  getLocalState: () => null,
  getStates: () => new Map(),
  setLocalState: () => {},
  setLocalStateField: () => {},
  on: () => {},
  off: () => {},
};

/**
 * Speaks the Yjs provider protocol over the notebook's GraphQL endpoints:
 * append-only `appendNotebookUpdates` out, polled `notebookUpdatesSince` in.
 * There is no WebSocket, so convergence is bounded by `POLL_INTERVAL_MS`.
 */
export function createGraphQLProvider(
  docId: string,
  docMap: Map<string, Y.Doc>,
  noteId: string,
  seedUpdates: { seq: number; update: string }[],
  fetcher: GraphQLFetcher,
): Provider {
  const doc = new Y.Doc();
  docMap.set(docId, doc);

  const listeners = new Map<string, Set<Listener>>();
  const emit = (type: string, ...args: unknown[]) =>
    listeners.get(type)?.forEach((cb) => cb(...args));

  let cursor = seedUpdates.reduce((max, row) => Math.max(max, row.seq), 0);
  // Server seqs already stored in this doc. The replay window deliberately re-serves
  // rows we hold; decoding and re-applying them every poll is pure waste.
  const applied = new Set(seedUpdates.map((row) => row.seq));
  let pending: Uint8Array[] = [];
  let flushTimer: ReturnType<typeof setInterval> | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  const persisted =
    typeof window === "undefined" ? null : pendingQueue(window.localStorage, noteId);

  const flush = async () => {
    if (pending.length === 0) return;
    const batch = pending;
    pending = [];
    try {
      await appendNotebookUpdates(fetcher, noteId, batch.map(toBase64));
      // Replace, not clear: updates pushed while the append was in flight are
      // already in `pending` and must stay in storage.
      persisted?.replace(pending);
    } catch {
      // Keep the bytes: a dropped update is a lost edit, and Yjs will dedupe
      // them on the server if a later attempt re-sends one that landed.
      pending = [...batch, ...pending];
    }
  };

  const poll = async () => {
    try {
      const rows = await fetchNotebookUpdates(
        fetcher,
        noteId,
        Math.max(0, cursor - REPLAY_WINDOW),
      );
      const fresh = rows.filter((row) => !applied.has(row.seq));
      for (const row of rows) cursor = Math.max(cursor, row.seq);
      if (fresh.length === 0) return;

      for (const row of fresh) applied.add(row.seq);
      // One merged update, one Yjs transaction, one Lexical reconciliation.
      Y.applyUpdate(doc, Y.mergeUpdates(fresh.map((row) => fromBase64(row.update))), REMOTE_ORIGIN);
    } catch {
      // Transient; the next tick retries from the same cursor.
    }
  };

  return {
    awareness: AWARENESS_STUB,
    connect() {
      // Merged, not applied one blob at a time: a note with a long history would
      // otherwise pay one Yjs transaction and one Lexical reconciliation per update.
      if (seedUpdates.length > 0) {
        Y.applyUpdate(
          doc,
          Y.mergeUpdates(seedUpdates.map((row) => fromBase64(row.update))),
        );
      }

      // Load before attaching the listener below, or draining would re-enqueue
      // updates the listener itself is about to push.
      pending = persisted?.drain() ?? [];

      // Attach only after the seed is replayed, or the seed itself would be
      // re-appended as fresh updates.
      doc.on("update", (update: Uint8Array, origin: unknown) => {
        if (origin === REMOTE_ORIGIN) return;
        pending.push(update);
        persisted?.push(update);
      });

      flushTimer = setInterval(() => void flush(), FLUSH_INTERVAL_MS);
      pollTimer = setInterval(() => void poll(), POLL_INTERVAL_MS);
      emit("sync", true);
    },
    disconnect() {
      if (flushTimer) clearInterval(flushTimer);
      if (pollTimer) clearInterval(pollTimer);
      flushTimer = null;
      pollTimer = null;
      void flush();
    },
    on(type: string, cb: Listener) {
      let set = listeners.get(type);
      if (!set) {
        set = new Set();
        listeners.set(type, set);
      }
      set.add(cb);
    },
    off(type: string, cb: Listener) {
      listeners.get(type)?.delete(cb);
    },
  } as Provider;
}
