import { fromBase64, toBase64 } from "./contract";

const keyFor = (noteId: string) => `tradstry-notebook-pending:${noteId}`;

/**
 * Local-storage-backed queue of un-flushed Yjs updates for one note.
 * `Storage` is injected so this stays usable outside a browser (SSR, tests).
 */
export function pendingQueue(storage: Storage, noteId: string) {
  const key = keyFor(noteId);

  const read = (): Uint8Array[] => {
    const raw = storage.getItem(key);
    if (!raw) return [];
    try {
      const parsed: unknown = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed.map((b64) => fromBase64(b64 as string));
    } catch {
      return [];
    }
  };

  const write = (updates: Uint8Array[]) => {
    storage.setItem(key, JSON.stringify(updates.map(toBase64)));
  };

  return {
    push(update: Uint8Array): void {
      write([...read(), update]);
    },
    drain(): Uint8Array[] {
      return read();
    },
    clear(): void {
      storage.removeItem(key);
    },
    replace(updates: Uint8Array[]): void {
      write(updates);
    },
  };
}
