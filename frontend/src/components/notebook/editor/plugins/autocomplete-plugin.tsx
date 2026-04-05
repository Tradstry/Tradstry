"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $isHeadingNode } from "@lexical/rich-text";
import {
  $createTextNode,
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_HIGH,
  KEY_ESCAPE_COMMAND,
  KEY_TAB_COMMAND,
} from "lexical";
import { useCallback, useEffect, useRef } from "react";
import type { GraphQLFetcher } from "@/lib/client";
import {
  $createGhostTextNode,
  $isGhostTextNode,
} from "../nodes/ghost-text-node";

const DEBOUNCE_MS = 200;

const AUTOCOMPLETE_MUTATION = `
  mutation NotebookAutocomplete($text: String!, $cursorPosition: Int!) {
    notebookAutocomplete(text: $text, cursorPosition: $cursorPosition)
  }
`;

function removeGhostNodes(editor: ReturnType<typeof useLexicalComposerContext>[0]) {
  editor.update(() => {
    const nodes = editor.getEditorState()._nodeMap;
    for (const [, node] of nodes) {
      if ($isGhostTextNode(node)) {
        node.remove();
      }
    }
  });
}

export function AutocompletePlugin({ fetcher }: { fetcher: GraphQLFetcher }) {
  const [editor] = useLexicalComposerContext();
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const ghostActiveRef = useRef(false);
  const requestIdRef = useRef(0);
  const suppressRef = useRef(false); // suppress update listener while we modify ghost nodes

  const clearGhost = useCallback(() => {
    if (ghostActiveRef.current) {
      suppressRef.current = true;
      removeGhostNodes(editor);
      ghostActiveRef.current = false;
      // Reset suppress after the update propagates
      requestAnimationFrame(() => { suppressRef.current = false; });
    }
  }, [editor]);

  // Listen for text changes → debounce → fetch
  useEffect(() => {
    return editor.registerUpdateListener(({ dirtyElements, dirtyLeaves, tags }) => {
      if (dirtyElements.size === 0 && dirtyLeaves.size === 0) return;
      // Skip updates caused by our own ghost node insertion/removal
      if (suppressRef.current) return;

      clearGhost();
      if (timerRef.current) clearTimeout(timerRef.current);
      const currentRequestId = ++requestIdRef.current;

      timerRef.current = setTimeout(() => {
        editor.getEditorState().read(() => {
          const selection = $getSelection();
          if (!$isRangeSelection(selection) || !selection.isCollapsed()) return;

          const anchor = selection.anchor;
          const node = anchor.getNode();
          const topLevel = node.getTopLevelElementOrThrow();

          // Skip H1 title
          if ($isHeadingNode(topLevel) && topLevel.getTag() === "h1") return;

          const text = topLevel.getTextContent();
          if (text.trim().length < 3) return;

          const offset = anchor.offset;

          fetcher<{ notebookAutocomplete: string }>(
            AUTOCOMPLETE_MUTATION,
            { text, cursorPosition: offset },
          ).then((data) => {
            // Stale response — user typed more since this request
            if (requestIdRef.current !== currentRequestId) return;

            const completion = data.notebookAutocomplete;
            if (!completion || completion.trim().length === 0) return;

            suppressRef.current = true;
            editor.update(() => {
              const currentSelection = $getSelection();
              if (!$isRangeSelection(currentSelection) || !currentSelection.isCollapsed()) return;

              const currentNode = currentSelection.anchor.getNode();
              const ghostNode = $createGhostTextNode(completion);
              currentNode.insertAfter(ghostNode);
              ghostActiveRef.current = true;
            });
            requestAnimationFrame(() => { suppressRef.current = false; });
          }).catch(() => {
            // Silently ignore autocomplete failures
          });
        });
      }, DEBOUNCE_MS);
    });
  }, [editor, clearGhost, fetcher]);

  // Tab to accept ghost text
  useEffect(() => {
    return editor.registerCommand(
      KEY_TAB_COMMAND,
      (event) => {
        if (!ghostActiveRef.current) return false;

        event.preventDefault();
        suppressRef.current = true;
        editor.update(() => {
          const nodes = editor.getEditorState()._nodeMap;
          for (const [, node] of nodes) {
            if ($isGhostTextNode(node)) {
              const text = node.getText();
              const textNode = $createTextNode(text);
              node.replace(textNode);
              textNode.selectEnd();
            }
          }
        });
        ghostActiveRef.current = false;
        requestAnimationFrame(() => { suppressRef.current = false; });
        return true;
      },
      COMMAND_PRIORITY_HIGH,
    );
  }, [editor]);

  // Escape to dismiss
  useEffect(() => {
    return editor.registerCommand(
      KEY_ESCAPE_COMMAND,
      () => {
        if (!ghostActiveRef.current) return false;
        clearGhost();
        return true;
      },
      COMMAND_PRIORITY_HIGH,
    );
  }, [editor, clearGhost]);

  // Cleanup
  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return null;
}
