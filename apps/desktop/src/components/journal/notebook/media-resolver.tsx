import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ensureMedia, type MediaResolved } from "../../../backend";

type MediaProgress = { hash: string; loaded: number; total: number };

export type ResolvedMedia = {
  src?: string;
  thumb?: string;
  pending: boolean;
  /** 0–100 while a download is streaming in; undefined when unknown (no
   *  Content-Length) or once the file is local. */
  percent?: number;
};

type MediaResolverContextValue = {
  resolve: (hash: string) => ResolvedMedia;
};

const MediaResolverContext = createContext<MediaResolverContextValue | null>(null);

/**
 * One resolver per open note. `ensureMedia` checks the on-disk cache first and
 * only reaches the network when the hash isn't there yet, so most `resolve`
 * calls settle on the first render pass. The in-flight ref keeps concurrent
 * decorate() calls for the same hash (multiple images sharing a paste) from
 * firing duplicate main-process invokes.
 */
export function MediaResolverProvider({
  noteId,
  children,
}: {
  noteId: string;
  children: ReactNode;
}) {
  const [results, setResults] = useState<Map<string, MediaResolved>>(new Map());
  const [progress, setProgress] = useState<Map<string, number>>(new Map());
  const inFlight = useRef<Set<string>>(new Set());

  useEffect(() => {
    const unlisten = window.tradstry.listen<MediaProgress>("notebook://media-progress", (event) => {
      const { hash, loaded, total } = event.payload;
      if (total <= 0) return; // indeterminate — leave percent undefined
      const percent = Math.min(100, Math.round((loaded / total) * 100));
      setProgress((prev) => new Map(prev).set(hash, percent));
    });
    return () => {
      unlisten();
    };
  }, []);

  const resolve = useCallback(
    (hash: string): ResolvedMedia => {
      const cached = results.get(hash);
      if (cached?.state === "local") {
        return {
          src: cached.fullPath ? window.tradstry.mediaUrl(cached.fullPath) : undefined,
          thumb: cached.thumbPath ? window.tradstry.mediaUrl(cached.thumbPath) : undefined,
          pending: false,
        };
      }

      if (!inFlight.current.has(hash)) {
        inFlight.current.add(hash);
        void ensureMedia(noteId, hash)
          .then((resolved) => {
            setResults((prev) => new Map(prev).set(hash, resolved));
          })
          .catch(() => {
            setResults((prev) =>
              new Map(prev).set(hash, {
                state: "missing",
                fullPath: null,
                thumbPath: null,
              }),
            );
          })
          .finally(() => {
            inFlight.current.delete(hash);
          });
      }

      return { pending: true, percent: progress.get(hash) };
    },
    [noteId, results, progress],
  );

  const value = useMemo(() => ({ resolve }), [resolve]);

  return (
    <MediaResolverContext.Provider value={value}>
      {children}
    </MediaResolverContext.Provider>
  );
}

export function useMediaResolver(): MediaResolverContextValue {
  const ctx = useContext(MediaResolverContext);
  if (!ctx) {
    throw new Error("useMediaResolver must be used within a MediaResolverProvider");
  }
  return ctx;
}
