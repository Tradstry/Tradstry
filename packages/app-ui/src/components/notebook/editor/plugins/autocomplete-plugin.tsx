"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $isHeadingNode } from "@lexical/rich-text";
import {
  $createTextNode,
  $getRoot,
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_HIGH,
  KEY_ESCAPE_COMMAND,
  KEY_TAB_COMMAND,
} from "lexical";
import { useCallback, useEffect, useRef } from "react";
import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import {
  $createGhostTextNode,
  $isGhostTextNode,
} from "../nodes/ghost-text-node";

const DEBOUNCE_MS = 200;
/** How much preceding note text to send as context — enough for topic and voice. */
const MAX_CONTEXT = 1200;

const AUTOCOMPLETE_MUTATION = `
  mutation NotebookAutocomplete($title: String!, $text: String!) {
    notebookAutocomplete(title: $title, text: $text)
  }
`;

function removeGhostNodes(
  editor: ReturnType<typeof useLexicalComposerContext>[0],
) {
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
      requestAnimationFrame(() => {
        suppressRef.current = false;
      });
    }
  }, [editor]);

  // Listen for text changes → debounce → fetch
  useEffect(() => {
    return editor.registerUpdateListener(({ dirtyElements, dirtyLeaves }) => {
      if (dirtyElements.size === 0 && dirtyLeaves.size === 0) return;
      // Skip updates caused by our own ghost node insertion/removal
      if (suppressRef.current) return;

      clearGhost();
      if (timerRef.current) clearTimeout(timerRef.current);
      const currentRequestId = ++requestIdRef.current;

      timerRef.current = setTimeout(() => {
        let vars: { title: string; text: string } | null = null;

        editor.getEditorState().read(() => {
          const selection = $getSelection();
          if (!$isRangeSelection(selection) || !selection.isCollapsed()) return;

          const anchor = selection.anchor;
          const node = anchor.getNode();
          const topLevel = node.getTopLevelElementOrThrow();

          // Skip the H1 title line — it's the note's title, not prose to continue.
          if ($isHeadingNode(topLevel) && topLevel.getTag() === "h1") return;

          const blockText = topLevel.getTextContent();
          if (blockText.trim().length < 3) return;

          // End-of-block only: bail if any text follows the caret in this block.
          // Ghosting mid-paragraph is where suggestions feel most out of place.
          let caretPos = anchor.offset;
          for (const textNode of topLevel.getAllTextNodes()) {
            if (textNode.getKey() === node.getKey()) break;
            caretPos += textNode.getTextContent().length;
          }
          if (caretPos < blockText.length) return;

          // Context = title + every preceding block + this block, capped to the
          // last MAX_CONTEXT chars so the model sees topic and voice, not a lone
          // fragment. Ghost nodes report empty text, so they never leak in.
          let title = "";
          let acc = "";
          for (const child of $getRoot().getChildren()) {
            if ($isHeadingNode(child) && child.getTag() === "h1") {
              title = child.getTextContent();
              continue;
            }
            if (child.getKey() === topLevel.getKey()) {
              acc += blockText;
              break;
            }
            const childText = child.getTextContent();
            if (childText.trim().length > 0) acc += `${childText}\n`;
          }

          vars = {
            title,
            text: acc.length > MAX_CONTEXT ? acc.slice(-MAX_CONTEXT) : acc,
          };
        });

        if (!vars) return;

        fetcher<{ notebookAutocomplete: string }>(AUTOCOMPLETE_MUTATION, vars)
          .then((data) => {
            // Stale response — user typed more since this request
            if (requestIdRef.current !== currentRequestId) return;

            const completion = data.notebookAutocomplete;
            if (!completion || completion.trim().length === 0) return;

            suppressRef.current = true;
            editor.update(() => {
              const currentSelection = $getSelection();
              if (
                !$isRangeSelection(currentSelection) ||
                !currentSelection.isCollapsed()
              )
                return;

              const currentNode = currentSelection.anchor.getNode();
              const ghostNode = $createGhostTextNode(completion);
              currentNode.insertAfter(ghostNode);
              ghostActiveRef.current = true;
            });
            requestAnimationFrame(() => {
              suppressRef.current = false;
            });
          })
          .catch(() => {
            // Silently ignore autocomplete failures
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
        requestAnimationFrame(() => {
          suppressRef.current = false;
        });
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
