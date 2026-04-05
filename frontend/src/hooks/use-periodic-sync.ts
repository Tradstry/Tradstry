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
 */
export function usePeriodicSync(
  onFlush: (noteId: string, change: PendingChange) => void,
  intervalMs = 5 * 60 * 1000,
) {
  const pendingRef = useRef<Record<string, PendingChange>>({});

  const flush = useCallback(() => {
    const pending = { ...pendingRef.current };
    pendingRef.current = {};

    for (const [noteId, change] of Object.entries(pending)) {
      onFlush(noteId, change);
    }
  }, [onFlush]);

  const enqueue = useCallback(
    (noteId: string, change: PendingChange) => {
      pendingRef.current[noteId] = change;
    },
    [],
  );

  // Periodic flush
  useEffect(() => {
    const id = setInterval(flush, intervalMs);
    return () => clearInterval(id);
  }, [flush, intervalMs]);

  // Flush on page leave + unmount
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
