"use client";

import { useCallback, useEffect, useRef } from "react";

type PendingChange = {
  documentJson: string;
  accountId: string;
};

// Near-real-time autosave tuning. IDLE = save this long after the user stops
// typing (the "feel"); MAX_WAIT = hard ceiling so continuous typing still
// flushes periodically (debounce alone would never fire while typing).
const IDLE_MS = 2_000;
const MAX_WAIT_MS = 10_000;

/**
 * Queues note changes and flushes them to a callback using a debounce +
 * max-wait scheduler (the autosave best-practice hybrid):
 *  - debounce: a flush fires ~IDLE_MS after the last edit, so a pause saves
 *    quickly without a request per keystroke;
 *  - max-wait: if edits keep coming, a flush is forced at least every
 *    MAX_WAIT_MS so a continuously-typing user never starves / loses work.
 * Changes are coalesced per note (only the latest `documentJson` is sent), so
 * a burst of edits collapses into a single backend write. Also flushes on tab
 * hide (visibilitychange), page leave (beforeunload), and unmount.
 *
 * `onFlush` is held in a ref so a parent can pass a fresh useCallback each
 * render without re-running the unmount cleanup (which intentionally flushes).
 */
export function usePeriodicSync(
  onFlush: (noteId: string, change: PendingChange) => void,
  {
    idleMs = IDLE_MS,
    maxWaitMs = MAX_WAIT_MS,
  }: { idleMs?: number; maxWaitMs?: number } = {},
) {
  const pendingRef = useRef<Record<string, PendingChange>>({});
  const onFlushRef = useRef(onFlush);
  const idleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const maxTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    onFlushRef.current = onFlush;
  }, [onFlush]);

  // Send every queued note. Clears both timers so the next edit starts a fresh
  // debounce + max-wait window.
  const flush = useCallback(() => {
    if (idleTimer.current) {
      clearTimeout(idleTimer.current);
      idleTimer.current = null;
    }
    if (maxTimer.current) {
      clearTimeout(maxTimer.current);
      maxTimer.current = null;
    }

    const pending = { ...pendingRef.current };
    pendingRef.current = {};
    for (const [noteId, change] of Object.entries(pending)) {
      onFlushRef.current(noteId, change);
    }
  }, []);

  // Debounce (reset idle timer each edit) + max-wait (a ceiling timer that is
  // only armed once per burst, so it isn't pushed back by continued typing).
  const schedule = useCallback(() => {
    if (idleTimer.current) clearTimeout(idleTimer.current);
    idleTimer.current = setTimeout(flush, idleMs);
    if (!maxTimer.current) {
      maxTimer.current = setTimeout(flush, maxWaitMs);
    }
  }, [flush, idleMs, maxWaitMs]);

  const enqueue = useCallback(
    (noteId: string, change: PendingChange) => {
      pendingRef.current[noteId] = change;
      schedule();
    },
    [schedule],
  );

  // Flush on tab hide, page leave, and unmount so nothing pending is lost.
  useEffect(() => {
    const handleBeforeUnload = () => flush();
    const handleVisibility = () => {
      if (document.visibilityState === "hidden") flush();
    };
    window.addEventListener("beforeunload", handleBeforeUnload);
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      window.removeEventListener("beforeunload", handleBeforeUnload);
      document.removeEventListener("visibilitychange", handleVisibility);
      flush();
    };
  }, [flush]);

  return { enqueue, flush };
}
