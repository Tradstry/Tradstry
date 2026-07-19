"use client";

import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $createParagraphNode,
  $createRangeSelection,
  $createTextNode,
  $getNodeByKey,
  $getRoot,
  $getSelection,
  $isRangeSelection,
  $setSelection,
  type PointType,
} from "lexical";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { GraphQLFetcher } from "@/lib/client";
import { showPlanLimit } from "@/lib/plan-limit";

type Action = "summarize" | "fix_spelling" | "simplify" | "expand";

const ACTIONS: { label: string; action: Action }[] = [
  { label: "Summarize", action: "summarize" },
  { label: "Fix Spelling", action: "fix_spelling" },
  { label: "Simplify", action: "simplify" },
  { label: "Expand", action: "expand" },
];

const CARD_TITLE: Record<Action, string> = {
  summarize: "Summary",
  fix_spelling: "Spelling fixed",
  simplify: "Simplified",
  expand: "Expanded",
};

const TRANSFORM_MUTATION = `
  mutation NotebookTransform($text: String!, $action: String!) {
    notebookTransform(text: $text, action: $action)
  }
`;

/** Viewport-relative box the card anchors to (the selection's rect). */
type Anchor = { top: number; bottom: number; centerX: number };
type Point = { top: number; left: number };
/** Enough to rebuild a RangeSelection over the same text after focus is lost. */
type PointSnapshot = { key: string; offset: number; type: "text" | "element" };

/**
 * A floating result card, never a destructive edit. `summarize` is read-only;
 * `expand` appends its text below `blockKey` on accept; `simplify`/`fix_spelling`
 * replace `range` on accept. Rejecting discards.
 */
type CardState = {
  action: Action;
  text: string;
  anchor: Anchor;
  blockKey: string | null;
  range: { anchor: PointSnapshot; focus: PointSnapshot } | null;
};

const snapshotPoint = (p: PointType): PointSnapshot => ({
  key: p.key,
  offset: p.offset,
  type: p.type,
});

export function SelectionToolbarPlugin({
  fetcher,
}: {
  fetcher: GraphQLFetcher;
}) {
  const [editor] = useLexicalComposerContext();
  const [show, setShow] = useState(false);
  const [position, setPosition] = useState<Point>({ top: 0, left: 0 });
  const [loading, setLoading] = useState<Action | null>(null);
  const [card, setCard] = useState<CardState | null>(null);
  const [cardPos, setCardPos] = useState<Point | null>(null);
  const [copied, setCopied] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const update = () => {
      // Keep the toolbar down while a result card is open.
      if (card !== null) {
        setShow(false);
        return;
      }

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

      const rect = nativeSel.getRangeAt(0).getBoundingClientRect();
      setPosition({
        top: rect.top - 40,
        left: rect.left + rect.width / 2,
      });
      setShow(true);
    };

    document.addEventListener("selectionchange", update);
    return () => document.removeEventListener("selectionchange", update);
  }, [editor, card]);

  // Position the card once it has rendered: prefer below the selection, flip
  // above when there's no room, and clamp fully inside the viewport. Measuring
  // the real card is what keeps a whole-note selection from parking it offscreen.
  useLayoutEffect(() => {
    if (!card || !cardRef.current) {
      setCardPos(null);
      return;
    }
    const { width, height } = cardRef.current.getBoundingClientRect();
    const margin = 12;
    const halfWidth = width / 2;
    const left = Math.max(
      halfWidth + margin,
      Math.min(card.anchor.centerX, window.innerWidth - halfWidth - margin),
    );

    let top = card.anchor.bottom + 8;
    if (top + height > window.innerHeight - margin) {
      const above = card.anchor.top - height - 8;
      top =
        above >= margin
          ? above
          : Math.max(margin, window.innerHeight - height - margin);
    }
    setCardPos({ top, left });
  }, [card]);

  const closeCard = useCallback(() => {
    setCard(null);
    setCardPos(null);
    setCopied(false);
  }, []);

  // Dismiss the floating card on Escape or a click anywhere outside it.
  useEffect(() => {
    if (card === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeCard();
    };
    const onPointerDown = (e: PointerEvent) => {
      if (cardRef.current && !cardRef.current.contains(e.target as Node)) {
        closeCard();
      }
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [card, closeCard]);

  const handleAction = useCallback(
    async (action: Action) => {
      let selectedText = "";
      let blockKey: string | null = null;
      let range: CardState["range"] = null;
      editor.getEditorState().read(() => {
        const sel = $getSelection();
        if ($isRangeSelection(sel)) {
          selectedText = sel.getTextContent();
          const nodes = sel.getNodes();
          const last = nodes[nodes.length - 1] ?? sel.focus.getNode();
          blockKey = last.getTopLevelElementOrThrow().getKey();
          range = {
            anchor: snapshotPoint(sel.anchor),
            focus: snapshotPoint(sel.focus),
          };
        }
      });

      if (!selectedText.trim()) return;

      // Capture the selection's box now — the card anchors here, and the
      // selection may be gone by the time the request resolves.
      const nativeSel = window.getSelection();
      const rect =
        nativeSel && nativeSel.rangeCount > 0
          ? nativeSel.getRangeAt(0).getBoundingClientRect()
          : null;
      const anchor: Anchor = rect
        ? {
            top: rect.top,
            bottom: rect.bottom,
            centerX: rect.left + rect.width / 2,
          }
        : { top: position.top, bottom: position.top, centerX: position.left };

      setLoading(action);

      try {
        const data = await fetcher<{ notebookTransform: string }>(
          TRANSFORM_MUTATION,
          { text: selectedText, action },
        );

        const result = data.notebookTransform;
        if (result?.trim()) {
          setCopied(false);
          setCard({ action, text: result.trim(), anchor, blockKey, range });
        }
      } catch (e) {
        if (!showPlanLimit(e)) {
          console.error("Selection transform error:", e);
        }
      } finally {
        setLoading(null);
        setShow(false);
      }
    },
    [editor, fetcher, position],
  );

  const copySummary = useCallback(async () => {
    if (!card) return;
    try {
      await navigator.clipboard.writeText(card.text);
      setCopied(true);
    } catch {
      // Clipboard unavailable (insecure context) — nothing to recover.
    }
  }, [card]);

  const acceptCard = useCallback(() => {
    if (!card) return;
    const { action, text, blockKey, range } = card;

    editor.update(() => {
      if (action === "expand") {
        // Append as new paragraphs below the block, leaving it untouched.
        const paragraphs = text
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean)
          .map((line) => {
            const p = $createParagraphNode();
            p.append($createTextNode(line));
            return p;
          });
        if (paragraphs.length === 0) return;

        const target = blockKey ? $getNodeByKey(blockKey) : null;
        if (target) {
          let anchor = target;
          for (const p of paragraphs) {
            anchor.insertAfter(p);
            anchor = p;
          }
        } else {
          const root = $getRoot();
          for (const p of paragraphs) root.append(p);
        }
        return;
      }

      // simplify / fix_spelling: replace exactly what was selected.
      if (!range) return;
      if (!$getNodeByKey(range.anchor.key) || !$getNodeByKey(range.focus.key)) {
        return;
      }
      const sel = $createRangeSelection();
      sel.anchor.set(range.anchor.key, range.anchor.offset, range.anchor.type);
      sel.focus.set(range.focus.key, range.focus.offset, range.focus.type);
      $setSelection(sel);
      const active = $getSelection();
      if ($isRangeSelection(active)) {
        active.insertRawText(text);
      }
    });

    closeCard();
  }, [card, editor, closeCard]);

  return (
    <>
      {show &&
        createPortal(
          <div
            className="fixed z-50 flex items-center gap-0.5 rounded-lg border border-border bg-popover px-1 py-0.5 shadow-lg"
            style={{
              top: position.top,
              left: position.left,
              transform: "translateX(-50%)",
            }}
          >
            {ACTIONS.map(({ label, action }) => (
              <button
                key={action}
                type="button"
                disabled={loading !== null}
                // Keep the text selection alive through the click, so the
                // transform still has a selection to read.
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleAction(action)}
                className="rounded-md px-2 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
              >
                {loading === action ? "..." : label}
              </button>
            ))}
          </div>,
          document.body,
        )}

      {card !== null &&
        createPortal(
          <div
            ref={cardRef}
            className="fixed z-50 flex w-[min(28rem,calc(100vw-2rem))] flex-col overflow-hidden rounded-xl border border-border bg-popover shadow-xl"
            style={{
              top: cardPos?.top ?? 0,
              left: cardPos?.left ?? 0,
              transform: "translateX(-50%)",
              visibility: cardPos ? "visible" : "hidden",
            }}
          >
            <div className="flex shrink-0 items-center justify-between gap-2 border-b border-border/60 px-3 py-2">
              <span className="text-xs font-semibold text-foreground">
                {CARD_TITLE[card.action]}
              </span>
              <button
                type="button"
                aria-label="Close"
                onClick={closeCard}
                className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                <HugeiconsIcon icon={Cancel01Icon} size={15} strokeWidth={2} />
              </button>
            </div>

            {/* Cap the viewport, not the root: the card is auto-height, so a
                root max-h can't scroll (see scroll-area.tsx). */}
            <ScrollArea className="[&>[data-radix-scroll-area-viewport]]:max-h-[50svh]">
              <p className="whitespace-pre-wrap px-3 py-2.5 text-sm leading-7 text-foreground/90">
                {card.text}
              </p>
            </ScrollArea>

            <div className="flex shrink-0 items-center justify-end gap-1.5 border-t border-border/60 px-3 py-2">
              {card.action === "summarize" ? (
                <button
                  type="button"
                  onClick={copySummary}
                  className="rounded-md px-2.5 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                >
                  {copied ? "Copied" : "Copy"}
                </button>
              ) : (
                <>
                  <button
                    type="button"
                    onClick={closeCard}
                    className="rounded-md px-2.5 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                  >
                    Reject
                  </button>
                  <button
                    type="button"
                    onClick={acceptCard}
                    className="rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                  >
                    Accept
                  </button>
                </>
              )}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
