"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $getRoot, $isElementNode, type LexicalNode } from "lexical";
import { useEffect, useRef } from "react";
import { getLocalBlob } from "../media-registry";
import {
  $isNotebookImageNode,
  useNotebookMediaActions,
} from "../nodes/notebook-image-node";
import { $isNotebookVideoNode } from "../nodes/notebook-video-node";

const POLL_MS = 4000;
const MAX_ATTEMPTS = 8;

/**
 * A media node references only a content hash; the URL is resolved from the
 * note's `images` list, which is a GraphQL query fetched when the note opened.
 * When a new image node arrives over the live CRDT (e.g. pasted on another
 * device), that list is stale and the node renders a pending box. This polls
 * `onRefresh` (a refetch of the notes query) while any media hash is unresolved,
 * then stops — bounded so a genuinely-missing image can't refetch forever.
 */
export function MediaRefreshPlugin({ onRefresh }: { onRefresh?: () => void }) {
  const [editor] = useLexicalComposerContext();
  const { urlFor } = useNotebookMediaActions();
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const attemptsRef = useRef(0);
  const signatureRef = useRef("");

  useEffect(() => {
    if (!onRefresh) {
      return;
    }

    const collectUnresolved = (): string[] => {
      const unresolved: string[] = [];
      editor.getEditorState().read(() => {
        const stack: LexicalNode[] = [...$getRoot().getChildren()];
        while (stack.length > 0) {
          const node = stack.pop();
          if (!node) continue;
          if ($isNotebookImageNode(node) || $isNotebookVideoNode(node)) {
            const hash = node.__hash;
            if (hash && !getLocalBlob(hash) && !urlFor?.(hash)) {
              unresolved.push(hash);
            }
          } else if ($isElementNode(node)) {
            stack.push(...node.getChildren());
          }
        }
      });
      return unresolved;
    };

    const tick = () => {
      timerRef.current = null;
      const unresolved = collectUnresolved();
      const signature = unresolved.slice().sort().join(",");
      // A new unresolved hash appeared — reset the attempt budget for it.
      if (signature !== signatureRef.current) {
        signatureRef.current = signature;
        attemptsRef.current = 0;
      }
      if (unresolved.length === 0 || attemptsRef.current >= MAX_ATTEMPTS) {
        return;
      }
      attemptsRef.current += 1;
      onRefresh();
      timerRef.current = setTimeout(tick, POLL_MS);
    };

    const schedule = () => {
      if (timerRef.current == null) {
        timerRef.current = setTimeout(tick, 1500);
      }
    };

    schedule();
    const unregister = editor.registerUpdateListener(schedule);
    return () => {
      unregister();
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [editor, urlFor, onRefresh]);

  return null;
}
