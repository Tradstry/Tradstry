"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  LexicalTypeaheadMenuPlugin,
  MenuOption,
  useBasicTypeaheadTriggerMatch,
} from "@lexical/react/LexicalTypeaheadMenuPlugin";
import { $isHeadingNode } from "@lexical/rich-text";
import {
  $createParagraphNode,
  $getRoot,
  $isParagraphNode,
  type LexicalNode,
  type ParagraphNode,
  type TextNode,
} from "lexical";
import { useCallback, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { JournalEntry } from "@/lib/types/journal";
import { cn, formatPnl } from "@/lib/utils";
import {
  $createLinkedTradeNode,
  $isLinkedTradeNode,
} from "../nodes/linked-trade-node";

class TradeMentionOption extends MenuOption {
  trade: JournalEntry;

  constructor(trade: JournalEntry) {
    super(trade.id);
    this.trade = trade;
  }
}

function clearQuery(textNodeContainingQuery: TextNode | null) {
  if (!textNodeContainingQuery) return;
  textNodeContainingQuery.selectStart();
  textNodeContainingQuery.setTextContent("");
}

/**
 * Walks the document looking for an existing LinkedTradeNode bound to
 * `tradeId`. Used to prevent the same trade from being linked twice.
 */
function findExistingChip(root: ReturnType<typeof $getRoot>, tradeId: string) {
  const stack: LexicalNode[] = [...root.getChildren()];
  while (stack.length > 0) {
    const node = stack.pop();
    if (!node) continue;
    if ($isLinkedTradeNode(node) && node.getTradeId() === tradeId) {
      return node;
    }
    if ("getChildren" in node && typeof node.getChildren === "function") {
      stack.push(...(node.getChildren() as LexicalNode[]));
    }
  }
  return null;
}

/**
 * Finds (or creates) the dedicated "chips paragraph" that sits between the
 * h1 title and the body. A chips paragraph is identified as the paragraph
 * immediately after the h1 whose children are all LinkedTradeNodes (or
 * empty text).
 */
function getOrCreateChipsParagraph(
  root: ReturnType<typeof $getRoot>,
): ParagraphNode {
  const children = root.getChildren();
  const h1Index = children.findIndex(
    (n) => $isHeadingNode(n) && n.getTag() === "h1",
  );

  // No h1 — insert a chips paragraph at the very start of the document.
  // (ensureNotebookStructure usually guarantees an h1; this is just safety.)
  if (h1Index === -1) {
    const p = $createParagraphNode();
    if (children.length > 0) {
      children[0].insertBefore(p);
    } else {
      root.append(p);
    }
    return p;
  }

  const h1 = children[h1Index];
  const next = children[h1Index + 1];

  if (next && $isParagraphNode(next)) {
    // Reuse the paragraph if it's already serving as the chips container
    // (i.e. every child is a LinkedTradeNode). Otherwise create a fresh one
    // so we don't pollute the user's body text.
    const grandchildren = next.getChildren() as LexicalNode[];
    const onlyChips =
      grandchildren.length > 0 &&
      grandchildren.every((c) => $isLinkedTradeNode(c));
    if (onlyChips) return next;
    if (grandchildren.length === 0) return next;
  }

  const p = $createParagraphNode();
  h1.insertAfter(p);
  return p;
}

export function AtMentionPlugin({
  trades = [],
  onLinkTrade,
}: {
  trades?: JournalEntry[];
  onLinkTrade?: (tradeId: string) => void;
}) {
  const [editor] = useLexicalComposerContext();
  const [queryString, setQueryString] = useState<string | null>(null);
  const checkForAtTriggerMatch = useBasicTypeaheadTriggerMatch("@", {
    minLength: 0,
  });

  const filteredTrades = useMemo(() => {
    const q = (queryString ?? "").trim().toLowerCase();
    if (!q) return trades.slice(0, 50);
    return trades
      .filter((t) => {
        const haystack = `${t.symbol} ${t.symbolName ?? ""}`.toLowerCase();
        return haystack.includes(q);
      })
      .slice(0, 50);
  }, [trades, queryString]);

  const options = useMemo(
    () => filteredTrades.map((t) => new TradeMentionOption(t)),
    [filteredTrades],
  );

  const handleSelect = useCallback(
    (
      option: TradeMentionOption,
      textNodeContainingQuery: TextNode | null,
      closeMenu: () => void,
    ) => {
      editor.update(() => {
        clearQuery(textNodeContainingQuery);

        const root = $getRoot();
        const existing = findExistingChip(root, option.trade.id);
        if (existing) {
          // Already linked. Skip insertion but still notify the parent
          // (no-op if the tradeId is already in the server-side list).
          if (onLinkTrade) onLinkTrade(option.trade.id);
          return;
        }

        const paragraph = getOrCreateChipsParagraph(root);
        paragraph.append($createLinkedTradeNode(option.trade.id));
      });

      if (onLinkTrade) onLinkTrade(option.trade.id);
      closeMenu();
    },
    [editor, onLinkTrade],
  );

  // When trades hasn't been triggered yet, queryString is null. Show the
  // menu the moment the user types `@` so they get visible feedback even if
  // their account has no trades (otherwise the trigger fires silently and
  // looks broken).
  const isOpen = queryString !== null;
  const hasNoTrades = isOpen && trades.length === 0;

  return (
    <LexicalTypeaheadMenuPlugin<TradeMentionOption>
      onQueryChange={setQueryString}
      triggerFn={checkForAtTriggerMatch}
      options={options}
      onSelectOption={handleSelect}
      anchorClassName="tradstry-at-mention-anchor"
      menuRenderFn={(
        anchorElementRef,
        { selectedIndex, setHighlightedIndex, selectOptionAndCleanUp },
      ) => {
        if (!anchorElementRef.current) return null;
        if (options.length === 0 && !hasNoTrades) return null;

        return createPortal(
          <div className="w-80 overflow-hidden rounded-2xl border border-border bg-popover p-2 shadow-2xl shadow-slate-900/10">
            <div className="shrink-0 px-2 pb-2 pt-1">
              <p className="text-[0.68rem] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                Link Trade
              </p>
            </div>
            {hasNoTrades ? (
              <div className="px-3 py-6 text-center">
                <p className="text-xs font-medium text-muted-foreground">
                  No trades to link
                </p>
                <p className="mt-1 text-[0.65rem] text-muted-foreground">
                  Create a journal entry first, then you can @-mention it here.
                </p>
              </div>
            ) : options.length === 0 ? (
              <div className="px-3 py-6 text-center">
                <p className="text-xs font-medium text-muted-foreground">
                  No matching trades
                </p>
                <p className="mt-1 text-[0.65rem] text-muted-foreground">
                  Try a different symbol.
                </p>
              </div>
            ) : null}
            <ScrollArea
              className="h-[min(20rem,calc(100vh-8rem))]"
              type="always"
              onWheelCapture={(e) => e.stopPropagation()}
            >
              <div className="space-y-1 px-1 pb-1 pr-3">
                {options.map((option, index) => {
                  const trade = option.trade;
                  const isProfit = trade.status === "profit";
                  return (
                    <button
                      key={option.key}
                      ref={option.setRefElement}
                      type="button"
                      className={cn(
                        "flex w-full items-center justify-between gap-2 rounded-xl px-3 py-2 text-left transition-colors",
                        selectedIndex === index
                          ? "bg-primary text-primary-foreground"
                          : "bg-transparent text-foreground hover:bg-accent",
                      )}
                      onMouseEnter={() => setHighlightedIndex(index)}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        setHighlightedIndex(index);
                        selectOptionAndCleanUp(option);
                      }}
                    >
                      <div className="flex min-w-0 flex-1 flex-col">
                        <span className="text-sm font-semibold tracking-wide">
                          {trade.symbol}
                        </span>
                        {trade.symbolName && (
                          <span
                            className={cn(
                              "truncate text-[0.7rem]",
                              selectedIndex === index
                                ? "text-muted-foreground"
                                : "text-muted-foreground",
                            )}
                          >
                            {trade.symbolName}
                          </span>
                        )}
                      </div>
                      <span
                        className={cn(
                          "rounded-md px-1.5 py-0.5 text-[0.65rem] font-medium tabular-nums",
                          selectedIndex === index
                            ? isProfit
                              ? "bg-emerald-500/30 text-emerald-100 dark:text-emerald-900"
                              : "bg-rose-500/30 text-rose-100 dark:text-rose-900"
                            : isProfit
                              ? "bg-emerald-50 text-emerald-700 dark:bg-emerald-950/50 dark:text-emerald-300"
                              : "bg-rose-50 text-rose-700 dark:bg-rose-950/50 dark:text-rose-300",
                        )}
                      >
                        {formatPnl(trade.totalPl, { precision: "cents" })}
                      </span>
                    </button>
                  );
                })}
              </div>
            </ScrollArea>
          </div>,
          anchorElementRef.current,
        );
      }}
    />
  );
}
