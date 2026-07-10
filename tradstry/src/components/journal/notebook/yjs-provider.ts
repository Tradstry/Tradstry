import * as Y from "yjs";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Provider } from "@lexical/yjs";
import { fromBase64, toBase64 } from "@tradstry/notebook-core";
import { appendNoteUpdate, noteUpdates } from "../../../backend";

/** Rust emits this after a sync stores new update blobs. Payload: note ids. */
const NOTE_UPDATES_EVENT = "notebook://updates";

type Listener = (...args: unknown[]) => void;

/**
 * Marks updates that came from the server. Apply every remote update with this
 * origin — the sync loop included — or it will be appended straight back.
 */
export const REMOTE_ORIGIN = Symbol("remote");

const AWARENESS_STUB = {
  getLocalState: () => null,
  getStates: () => new Map(),
  setLocalState: () => {},
  setLocalStateField: () => {},
  on: () => {},
  off: () => {},
};

/**
 * A local shim standing in for a WebSocket provider: the webview must never
 * touch the network — the auth token lives only in Rust — so updates travel
 * through Tauri commands, and the sync loop ships the outbox rows.
 */
export function createLocalProvider(
  docId: string,
  docMap: Map<string, Y.Doc>,
  noteId: string,
  seedUpdatesB64: string[],
): Provider {
  const doc = new Y.Doc();
  docMap.set(docId, doc);

  const listeners = new Map<string, Set<Listener>>();
  const emit = (type: string, ...args: unknown[]) =>
    listeners.get(type)?.forEach((cb) => cb(...args));

  let unlisten: UnlistenFn | null = null;
  let disconnected = false;

  // Re-applying an update the doc already holds is a no-op that fires no `update`
  // event, so replaying the whole local chain costs time but cannot echo back.
  const applyLocalChain = async () => {
    const blobs = await noteUpdates(noteId);
    if (blobs.length === 0) return;
    Y.applyUpdate(doc, Y.mergeUpdates(blobs.map(fromBase64)), REMOTE_ORIGIN);
  };

  return {
    awareness: AWARENESS_STUB,
    connect() {
      // Merged, not one blob at a time: a long history would otherwise pay one Yjs
      // transaction and one Lexical reconciliation per update.
      if (seedUpdatesB64.length > 0) {
        Y.applyUpdate(
          doc,
          Y.mergeUpdates(seedUpdatesB64.map(fromBase64)),
          REMOTE_ORIGIN,
        );
      }
      // Attach the persistence handler only after the seed is replayed, or the
      // seed itself would be re-enqueued as fresh outbox rows.
      doc.on("update", (update: Uint8Array, origin: unknown) => {
        // Anything the server sent us must never be sent back: it would be
        // re-appended, arrive again on the next pull, and grow the note's update
        // log without bound. `observeDeep` still syncs it into Lexical, because
        // that only skips updates whose origin is the binding itself.
        if (origin === REMOTE_ORIGIN) return;
        void appendNoteUpdate(noteId, toBase64(update));
      });

      // The sync loop writes remote updates into the local store; without this an
      // open note would not see another device's edits until it was reopened.
      void listen<string[]>(NOTE_UPDATES_EVENT, (event) => {
        if (!event.payload.includes(noteId)) return;
        void applyLocalChain();
      }).then((fn) => {
        if (disconnected) fn();
        else unlisten = fn;
      });

      emit("sync", true);
    },
    disconnect() {
      disconnected = true;
      unlisten?.();
      unlisten = null;
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
