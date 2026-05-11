"use client";

import { useCallback, useEffect, useRef } from "react";

type PendingChange = {
  documentJson: string;
  accountId: string;
};

/**
 * Queues note changes and flushes them to a callback on a fixed interval.
 * Also flushes on page leave (beforeunload) and component unmount.
 * localStorage saving is handled separately by the editor — this hook
 * only manages the backend sync schedule.
 *
 * The onFlush prop is held in a ref so it can change on every render
 * (e.g. when a parent passes a fresh useCallback) without re-running the
 * cleanup-on-unmount effect — that cleanup intentionally calls flush(),
 * and a stable internal flush identity is what keeps the cleanup from
 * firing spuriously on every re-render.
 */
export function usePeriodicSync(
  onFlush: (noteId: string, change: PendingChange) => void,
  intervalMs = 5 * 60 * 1000,
) {
  const pendingRef = useRef<Record<string, PendingChange>>({});
  const onFlushRef = useRef(onFlush);

  // Keep ref pointed at the latest onFlush each render, without using it as
  // a dependency for any effect.
  useEffect(() => {
    onFlushRef.current = onFlush;
  }, [onFlush]);

  const flush = useCallback(() => {
    const pending = { ...pendingRef.current };
    pendingRef.current = {};

    for (const [noteId, change] of Object.entries(pending)) {
      onFlushRef.current(noteId, change);
    }
  }, []);

  const enqueue = useCallback((noteId: string, change: PendingChange) => {
    pendingRef.current[noteId] = change;
  }, []);

  // Periodic flush
  useEffect(() => {
    const id = setInterval(flush, intervalMs);
    return () => clearInterval(id);
  }, [flush, intervalMs]);

  // Flush on page leave + unmount. `flush` is stable now, so this effect
  // sets up exactly once per mount and tears down exactly once.
  useEffect(() => {
    const handleBeforeUnload = () => flush();
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => {
      window.removeEventListener("beforeunload", handleBeforeUnload);
      flush();
    };
  }, [flush]);

  return { enqueue, flush };
}
