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
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import type { JournalEntry } from "@tradstry/app-ui/lib/types/journal";
import { cn, formatPnl } from "@tradstry/app-ui/lib/utils";
import {
  $createLinkedTradeNode,
  $isLinkedTradeNode,
} from "../nodes/linked-trade-node";

function shortDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

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
  onBrowseAll,
}: {
  trades?: JournalEntry[];
  onLinkTrade?: (tradeId: string) => void;
  /** Escalates to the full picker, carrying whatever was typed after `@`. */
  onBrowseAll?: (query: string) => void;
}) {
  const [editor] = useLexicalComposerContext();
  const [queryString, setQueryString] = useState<string | null>(null);
  const checkForAtTriggerMatch = useBasicTypeaheadTriggerMatch("@", {
    minLength: 0,
  });

  // Most recent first: the trade you're writing about is almost always a recent one.
  const recentTrades = useMemo(
    () =>
      [...trades].sort(
        (a, b) =>
          new Date(b.openDate).getTime() - new Date(a.openDate).getTime(),
      ),
    [trades],
  );

  const filteredTrades = useMemo(() => {
    const q = (queryString ?? "").trim().toLowerCase();
    if (!q) return recentTrades.slice(0, 50);
    return recentTrades
      .filter((t) => {
        const tags = t.tags?.map((tag) => tag.name).join(" ") ?? "";
        const haystack =
          `${t.symbol} ${t.symbolName ?? ""} ${t.tradeType} ${tags}`.toLowerCase();
        return haystack.includes(q);
      })
      .slice(0, 50);
  }, [recentTrades, queryString]);

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
        // Keep the menu open on zero matches when there is still a "Browse all"
        // row to offer — a failed symbol search is the moment you want the dialog.
        if (options.length === 0 && !hasNoTrades && !onBrowseAll) return null;

        return createPortal(
          <div className="w-96 overflow-hidden rounded-2xl border border-border bg-popover p-2 shadow-2xl shadow-slate-900/10">
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
                        {/* Side and open date are what separate two trades on the
                            same symbol; entry→exit confirms it's the right one. */}
                        <span className="flex min-w-0 items-baseline gap-1.5 text-sm font-semibold tracking-wide">
                          <span className="truncate">{trade.symbol}</span>
                          <span
                            className={cn(
                              "shrink-0 text-[0.6rem] font-medium uppercase tracking-wide",
                              selectedIndex === index
                                ? "opacity-80"
                                : trade.tradeType === "short"
                                  ? "text-amber-600 dark:text-amber-400"
                                  : "text-sky-600 dark:text-sky-400",
                            )}
                          >
                            {trade.tradeType}
                          </span>
                          <span className="shrink-0 text-[0.65rem] font-normal opacity-70">
                            {shortDate(trade.openDate)}
                          </span>
                        </span>
                        <span
                          className={cn(
                            "truncate text-[0.7rem] tabular-nums",
                            selectedIndex === index
                              ? "opacity-80"
                              : "text-muted-foreground",
                          )}
                        >
                          {trade.entryPrice} → {trade.exitPrice}
                          {trade.symbolName ? ` · ${trade.symbolName}` : ""}
                        </span>
                      </div>
                      <span
                        className={cn(
                          "shrink-0 rounded-md px-1.5 py-0.5 text-[0.65rem] font-medium tabular-nums",
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
            {onBrowseAll ? (
              <button
                type="button"
                onMouseDown={(e) => {
                  e.preventDefault();
                  onBrowseAll(queryString ?? "");
                }}
                className="mt-1 flex w-full items-center justify-between gap-2 rounded-xl border-t border-border px-3 py-2 text-left text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                <span>Browse all {trades.length} trades…</span>
                <span className="text-[0.65rem] opacity-60">
                  insert as table
                </span>
              </button>
            ) : null}
          </div>,
          anchorElementRef.current,
        );
      }}
    />
  );
}
