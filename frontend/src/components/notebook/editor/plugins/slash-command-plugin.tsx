"use client";

import { $createCodeNode } from "@lexical/code";
import {
  INSERT_ORDERED_LIST_COMMAND,
  INSERT_UNORDERED_LIST_COMMAND,
} from "@lexical/list";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $createHorizontalRuleNode } from "@lexical/react/LexicalHorizontalRuleNode";
import {
  LexicalTypeaheadMenuPlugin,
  MenuOption,
  useBasicTypeaheadTriggerMatch,
} from "@lexical/react/LexicalTypeaheadMenuPlugin";
import { $createHeadingNode, $createQuoteNode } from "@lexical/rich-text";
import { $setBlocksType } from "@lexical/selection";
import { $insertNodeToNearestRoot } from "@lexical/utils";
import {
  $createParagraphNode,
  $getSelection,
  $isRangeSelection,
  FORMAT_ELEMENT_COMMAND,
  FORMAT_TEXT_COMMAND,
  type LexicalEditor,
  type TextNode,
} from "lexical";
import { useCallback, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { JournalEntry } from "@/lib/types/journal";
import { cn } from "@/lib/utils";

class SlashCommandOption extends MenuOption {
  description: string;
  group: string;
  keywords: string[];
  onSelect: (editor: LexicalEditor) => void;

  constructor(
    title: string,
    group: string,
    description: string,
    keywords: string[],
    onSelect: (editor: LexicalEditor) => void,
  ) {
    super(title);
    this.group = group;
    this.description = description;
    this.keywords = keywords;
    this.onSelect = onSelect;
  }
}

function clearQuery(textNodeContainingQuery: TextNode | null) {
  if (!textNodeContainingQuery) {
    return;
  }

  textNodeContainingQuery.selectStart();
  textNodeContainingQuery.setTextContent("");
}

export function SlashCommandPlugin({
  trades = [],
  onLinkTrade,
}: {
  trades?: JournalEntry[];
  onLinkTrade?: (tradeId: string) => void;
} = {}) {
  const [editor] = useLexicalComposerContext();
  const [queryString, setQueryString] = useState<string | null>(null);
  const [showTradePicker, setShowTradePicker] = useState(false);
  const [tradePickerAnchor, setTradePickerAnchor] =
    useState<HTMLElement | null>(null);
  const [tradeQuery, setTradeQuery] = useState("");
  const checkForSlashTriggerMatch = useBasicTypeaheadTriggerMatch("/", {
    minLength: 0,
  });

  const filteredTrades = useMemo(() => {
    const q = tradeQuery.toLowerCase();
    return trades.filter(
      (t) =>
        t.symbol.toLowerCase().includes(q) ||
        t.symbolName.toLowerCase().includes(q),
    );
  }, [trades, tradeQuery]);

  const handleSelectTrade = useCallback(
    (trade: JournalEntry) => {
      setShowTradePicker(false);
      setTradeQuery("");
      onLinkTrade?.(trade.id);
    },
    [onLinkTrade],
  );

  const options = useMemo(
    () => [
      new SlashCommandOption(
        "Text",
        "Basic blocks",
        "Convert the current block back to plain paragraph text",
        ["paragraph", "plain", "text"],
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            $setBlocksType(selection, () => $createParagraphNode());
          }
        },
      ),
      new SlashCommandOption(
        "Heading 1",
        "Basic blocks",
        "Create a large page heading",
        ["h1", "title", "heading"],
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            $setBlocksType(selection, () => $createHeadingNode("h1"));
          }
        },
      ),
      new SlashCommandOption(
        "Heading 2",
        "Basic blocks",
        "Create a section heading",
        ["h2", "section", "heading"],
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            $setBlocksType(selection, () => $createHeadingNode("h2"));
          }
        },
      ),
      new SlashCommandOption(
        "Heading 3",
        "Basic blocks",
        "Create a compact subsection heading",
        ["h3", "subheading", "heading"],
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            $setBlocksType(selection, () => $createHeadingNode("h3"));
          }
        },
      ),
      new SlashCommandOption(
        "Heading 4",
        "Basic blocks",
        "Create a small utility heading",
        ["h4", "small heading", "heading"],
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            $setBlocksType(selection, () => $createHeadingNode("h4"));
          }
        },
      ),
      new SlashCommandOption(
        "Bullet List",
        "Lists",
        "Start an unordered list",
        ["list", "bullet", "unordered"],
        (editor) => {
          editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined);
        },
      ),
      new SlashCommandOption(
        "Numbered List",
        "Lists",
        "Start an ordered list",
        ["list", "ordered", "numbered"],
        (editor) => {
          editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND, undefined);
        },
      ),
      new SlashCommandOption(
        "Quote",
        "Basic blocks",
        "Insert a quoted callout block",
        ["quote", "blockquote", "callout"],
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            $setBlocksType(selection, () => $createQuoteNode());
          }
        },
      ),
      new SlashCommandOption(
        "Code Block",
        "Basic blocks",
        "Insert a multiline code block",
        ["code", "snippet", "block"],
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            $setBlocksType(selection, () => $createCodeNode());
          }
        },
      ),
      new SlashCommandOption(
        "Divider",
        "Basic blocks",
        "Insert a horizontal divider line",
        ["divider", "separator", "rule", "line"],
        () => {
          $insertNodeToNearestRoot($createHorizontalRuleNode());
        },
      ),
      new SlashCommandOption(
        "Bold",
        "Formatting",
        "Toggle bold formatting on the current selection",
        ["strong", "bold", "format"],
        (editor) => {
          editor.dispatchCommand(FORMAT_TEXT_COMMAND, "bold");
        },
      ),
      new SlashCommandOption(
        "Italic",
        "Formatting",
        "Toggle italic formatting on the current selection",
        ["italic", "emphasis", "format"],
        (editor) => {
          editor.dispatchCommand(FORMAT_TEXT_COMMAND, "italic");
        },
      ),
      new SlashCommandOption(
        "Underline",
        "Formatting",
        "Toggle underline formatting on the current selection",
        ["underline", "format"],
        (editor) => {
          editor.dispatchCommand(FORMAT_TEXT_COMMAND, "underline");
        },
      ),
      new SlashCommandOption(
        "Strikethrough",
        "Formatting",
        "Toggle strikethrough formatting on the current selection",
        ["strike", "delete", "format"],
        (editor) => {
          editor.dispatchCommand(FORMAT_TEXT_COMMAND, "strikethrough");
        },
      ),
      new SlashCommandOption(
        "Inline Code",
        "Formatting",
        "Toggle inline code formatting on the current selection",
        ["inline code", "code", "format"],
        (editor) => {
          editor.dispatchCommand(FORMAT_TEXT_COMMAND, "code");
        },
      ),
      new SlashCommandOption(
        "Align Left",
        "Layout",
        "Align the current block to the left",
        ["align left", "left", "layout"],
        (editor) => {
          editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "left");
        },
      ),
      new SlashCommandOption(
        "Align Center",
        "Layout",
        "Center the current block",
        ["align center", "center", "layout"],
        (editor) => {
          editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "center");
        },
      ),
      new SlashCommandOption(
        "Align Right",
        "Layout",
        "Align the current block to the right",
        ["align right", "right", "layout"],
        (editor) => {
          editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "right");
        },
      ),
      new SlashCommandOption(
        "Justify",
        "Layout",
        "Justify the current block",
        ["justify", "layout", "alignment"],
        (editor) => {
          editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "justify");
        },
      ),
      ...(trades.length > 0
        ? [
            new SlashCommandOption(
              "Link Trade",
              "Trade",
              "Insert a trade summary block and link it to this note",
              ["trade", "link", "journal", "position"],
              () => {
                setShowTradePicker(true);
                setTradeQuery("");
              },
            ),
          ]
        : []),
    ],
    [trades],
  );

  const filteredOptions = useMemo(() => {
    const query = (queryString ?? "").trim().toLowerCase();
    if (!query) {
      return options;
    }

    return options.filter((option) => {
      const haystack = [option.key, option.description, ...option.keywords]
        .join(" ")
        .toLowerCase();
      return haystack.includes(query);
    });
  }, [options, queryString]);

  const groupedOptions = useMemo(() => {
    const groups = new Map<string, SlashCommandOption[]>();

    for (const option of filteredOptions) {
      const current = groups.get(option.group) ?? [];
      current.push(option);
      groups.set(option.group, current);
    }

    return Array.from(groups.entries());
  }, [filteredOptions]);

  return (
    <>
      <LexicalTypeaheadMenuPlugin
        onQueryChange={setQueryString}
        triggerFn={checkForSlashTriggerMatch}
        options={filteredOptions}
        anchorClassName="tradstry-slash-anchor"
        onSelectOption={(
          selectedOption,
          textNodeContainingQuery,
          closeMenu,
        ) => {
          clearQuery(textNodeContainingQuery);
          selectedOption.onSelect(editor);
          closeMenu();
        }}
        menuRenderFn={(
          anchorElementRef,
          {
            options,
            selectedIndex,
            setHighlightedIndex,
            selectOptionAndCleanUp,
          },
        ) => {
          if (!anchorElementRef.current || options.length === 0) {
            return null;
          }

          return createPortal(
            <div className="w-80 overflow-hidden rounded-2xl border border-slate-200 bg-white p-2 shadow-2xl shadow-slate-900/10">
              <div className="shrink-0 px-2 pb-2 pt-1">
                <p className="text-[0.68rem] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                  Slash Commands
                </p>
              </div>
              <ScrollArea
                className="h-[min(24rem,calc(100vh-8rem))]"
                type="always"
                onWheelCapture={(event) => {
                  event.stopPropagation();
                }}
              >
                <div className="space-y-3 px-1 pb-1 pr-3">
                  {groupedOptions.map(([group, groupOptions]) => (
                    <div key={group} className="space-y-1">
                      <p className="px-2 text-[0.62rem] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
                        {group}
                      </p>
                      {groupOptions.map((option) => {
                        const index = options.findIndex(
                          (currentOption) => currentOption.key === option.key,
                        );

                        return (
                          <button
                            key={option.key}
                            ref={option.setRefElement}
                            type="button"
                            className={cn(
                              "flex w-full flex-col rounded-xl px-3 py-2 text-left transition-colors",
                              selectedIndex === index
                                ? "bg-slate-900 text-white"
                                : "bg-transparent text-slate-900 hover:bg-slate-100",
                            )}
                            onMouseEnter={() => setHighlightedIndex(index)}
                            onMouseDown={(event) => {
                              event.preventDefault();
                              setHighlightedIndex(index);
                              selectOptionAndCleanUp(option);
                            }}
                          >
                            <span className="text-sm font-medium">
                              {option.key}
                            </span>
                            <span
                              className={cn(
                                "mt-0.5 text-xs",
                                selectedIndex === index
                                  ? "text-slate-300"
                                  : "text-muted-foreground",
                              )}
                            >
                              {option.description}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  ))}
                </div>
              </ScrollArea>
            </div>,
            anchorElementRef.current,
          );
        }}
      />
      {showTradePicker && (
        <TradePickerOverlay
          trades={filteredTrades}
          query={tradeQuery}
          onQueryChange={setTradeQuery}
          onSelect={handleSelectTrade}
          onClose={() => {
            setShowTradePicker(false);
            setTradeQuery("");
          }}
        />
      )}
    </>
  );
}

function TradePickerOverlay({
  trades,
  query,
  onQueryChange,
  onSelect,
  onClose,
}: {
  trades: JournalEntry[];
  query: string;
  onQueryChange: (q: string) => void;
  onSelect: (trade: JournalEntry) => void;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
      onMouseDown={onClose}
    >
      <div
        className="w-96 overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xl shadow-slate-900/10"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="border-b border-slate-100 px-3 py-2">
          <input
            type="text"
            value={query}
            onChange={(e) => onQueryChange(e.target.value)}
            placeholder="Search trades by symbol..."
            className="w-full bg-transparent text-sm text-slate-900 placeholder:text-slate-400 focus:outline-none"
            autoFocus
          />
        </div>
        <ScrollArea className="max-h-64">
          {trades.length === 0 ? (
            <div className="px-3 py-4 text-center text-xs text-slate-400">
              No trades found
            </div>
          ) : (
            trades.slice(0, 20).map((trade) => {
              const plSign = trade.totalPl >= 0 ? "+" : "";
              return (
                <button
                  key={trade.id}
                  type="button"
                  className="flex w-full items-center justify-between px-3 py-2.5 text-left transition-colors hover:bg-slate-50"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    onSelect(trade);
                  }}
                >
                  <div>
                    <span className="text-sm font-medium text-slate-900">
                      {trade.symbol}
                    </span>
                    <span className="ml-2 text-xs text-slate-400">
                      {trade.tradeType} · {trade.openDate}
                    </span>
                  </div>
                  <span
                    className={cn(
                      "text-sm font-medium",
                      trade.status === "profit"
                        ? "text-emerald-600"
                        : "text-red-500",
                    )}
                  >
                    {plSign}${trade.totalPl.toFixed(2)}
                  </span>
                </button>
              );
            })
          )}
        </ScrollArea>
      </div>
    </div>
  );
}
