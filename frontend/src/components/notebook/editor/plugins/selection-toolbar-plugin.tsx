"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
} from "lexical";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { GraphQLFetcher } from "@/lib/client";

type Action = "summarize" | "fix_spelling" | "simplify" | "expand";

const ACTIONS: { label: string; action: Action }[] = [
  { label: "Summarize", action: "summarize" },
  { label: "Fix Spelling", action: "fix_spelling" },
  { label: "Simplify", action: "simplify" },
  { label: "Expand", action: "expand" },
];

const TRANSFORM_MUTATION = `
  mutation NotebookTransform($text: String!, $action: String!) {
    notebookTransform(text: $text, action: $action)
  }
`;

export function SelectionToolbarPlugin({ fetcher }: { fetcher: GraphQLFetcher }) {
  const [editor] = useLexicalComposerContext();
  const [show, setShow] = useState(false);
  const [position, setPosition] = useState<{ top: number; left: number }>({ top: 0, left: 0 });
  const [loading, setLoading] = useState<Action | null>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const update = () => {
      const nativeSel = window.getSelection();
      if (!nativeSel || nativeSel.isCollapsed || !nativeSel.rangeCount) {
        setShow(false);
        return;
      }

      const text = nativeSel.toString().trim();
      if (text.length < 2) {
        setShow(false);
        return;
      }

      const rootEl = editor.getRootElement();
      if (!rootEl || !rootEl.contains(nativeSel.anchorNode)) {
        setShow(false);
        return;
      }

      const range = nativeSel.getRangeAt(0);
      const rect = range.getBoundingClientRect();

      setPosition({
        top: rect.top + window.scrollY - 40,
        left: rect.left + window.scrollX + rect.width / 2,
      });
      setShow(true);
    };

    document.addEventListener("selectionchange", update);
    return () => document.removeEventListener("selectionchange", update);
  }, [editor]);

  const handleAction = useCallback(
    async (action: Action) => {
      let selectedText = "";
      editor.getEditorState().read(() => {
        const sel = $getSelection();
        if ($isRangeSelection(sel)) {
          selectedText = sel.getTextContent();
        }
      });

      if (!selectedText.trim()) return;

      setLoading(action);

      try {
        const data = await fetcher<{ notebookTransform: string }>(
          TRANSFORM_MUTATION,
          { text: selectedText, action },
        );

        const result = data.notebookTransform;
        if (result && result.trim()) {
          editor.update(() => {
            const sel = $getSelection();
            if ($isRangeSelection(sel)) {
              sel.insertRawText(result);
            }
          });
        }
      } catch (e) {
        console.error("Selection transform error:", e);
      } finally {
        setLoading(null);
        setShow(false);
      }
    },
    [editor, fetcher],
  );

  if (!show) return null;

  return createPortal(
    <div
      ref={toolbarRef}
      className="fixed z-50 flex items-center gap-0.5 rounded-lg border border-slate-200 bg-white px-1 py-0.5 shadow-lg"
      style={{
        top: position.top,
        left: position.left,
        transform: "translateX(-50%)",
      }}
      onMouseDown={(e) => e.preventDefault()}
    >
      {ACTIONS.map(({ label, action }) => (
        <button
          key={action}
          type="button"
          disabled={loading !== null}
          onClick={() => handleAction(action)}
          className="rounded-md px-2 py-1 text-xs font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 disabled:opacity-50"
        >
          {loading === action ? "..." : label}
        </button>
      ))}
    </div>,
    document.body,
  );
}
