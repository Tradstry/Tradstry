"use client";

import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  type SerializedTradeTableNode,
  TradeTableNode as TradeTableSchema,
} from "@tradstry/notebook-core";
import { $getNodeByKey, type LexicalNode, type NodeKey } from "lexical";
import { type ReactNode, useCallback, useContext, useState } from "react";
import type { JournalEntry } from "@tradstry/app-ui/lib/types/journal";
import { cn, formatPnl } from "@tradstry/app-ui/lib/utils";
import { LinkedTradeContext } from "./linked-trade-node";

const COLLAPSED_ROWS = 5;

function shortDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function TradeTableRow({
  trade,
  onRemove,
}: {
  trade: JournalEntry;
  onRemove: () => void;
}) {
  const isProfit = trade.status === "profit";

  return (
    <div className="group/row flex items-center gap-3 px-3 py-1.5 text-xs hover:bg-accent/50">
      <span className="w-16 shrink-0 font-semibold tracking-wide">
        {trade.symbol}
      </span>
      <span
        className={cn(
          "w-12 shrink-0 text-[0.65rem] uppercase tracking-wide",
          trade.tradeType === "short"
            ? "text-amber-600 dark:text-amber-400"
            : "text-sky-600 dark:text-sky-400",
        )}
      >
        {trade.tradeType}
      </span>
      <span className="w-14 shrink-0 text-muted-foreground">
        {shortDate(trade.openDate)}
      </span>
      <span className="flex-1 truncate tabular-nums text-muted-foreground">
        {trade.entryPrice} → {trade.exitPrice}
      </span>
      <span
        className={cn(
          "shrink-0 tabular-nums font-medium",
          isProfit
            ? "text-emerald-600 dark:text-emerald-400"
            : "text-rose-600 dark:text-rose-400",
        )}
      >
        {formatPnl(trade.totalPl, { precision: "cents" })}
      </span>
      <button
        type="button"
        onClick={onRemove}
        title="Remove from table"
        className="shrink-0 rounded p-0.5 text-muted-foreground opacity-0 transition-opacity hover:bg-muted hover:text-foreground group-hover/row:opacity-100"
      >
        <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
      </button>
    </div>
  );
}

/** A trade that was deleted from the journal. Shown rather than dropped, so a
 *  review never silently loses a row. */
function MissingTradeRow({ onRemove }: { onRemove: () => void }) {
  return (
    <div className="group/row flex items-center gap-3 px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent/50">
      <span className="flex-1 italic">Trade no longer available</span>
      <button
        type="button"
        onClick={onRemove}
        title="Remove from table"
        className="shrink-0 rounded p-0.5 opacity-0 transition-opacity hover:bg-muted hover:text-foreground group-hover/row:opacity-100"
      >
        <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
      </button>
    </div>
  );
}

function TradeTable({
  nodeKey,
  tradeIds,
  label,
}: {
  nodeKey: NodeKey;
  tradeIds: string[];
  label: string;
}) {
  const [editor] = useLexicalComposerContext();
  const { trades, onUnlinkTrade } = useContext(LinkedTradeContext);
  const [expanded, setExpanded] = useState(false);

  const resolved = tradeIds.map((id) => ({
    id,
    trade: trades.find((t) => t.id === id),
  }));
  const net = resolved.reduce((sum, r) => sum + (r.trade?.totalPl ?? 0), 0);

  const removeTrade = useCallback(
    (tradeId: string) => {
      editor.update(() => {
        const node = $getNodeByKey(nodeKey);
        if (!$isTradeTableNode(node)) return;
        const remaining = node.getTradeIds().filter((id) => id !== tradeId);
        if (remaining.length === 0) {
          node.remove();
        } else {
          node.setTradeIds(remaining);
        }
      });
      onUnlinkTrade?.(tradeId);
    },
    [editor, nodeKey, onUnlinkTrade],
  );

  const setLabel = useCallback(
    (next: string) => {
      editor.update(() => {
        const node = $getNodeByKey(nodeKey);
        if ($isTradeTableNode(node)) node.setLabel(next);
      });
    },
    [editor, nodeKey],
  );

  const visible = expanded ? resolved : resolved.slice(0, COLLAPSED_ROWS);
  const hidden = resolved.length - visible.length;

  return (
    <div
      contentEditable={false}
      className="my-3 overflow-hidden rounded-xl border border-border bg-card"
    >
      <div className="flex items-center justify-between gap-3 border-b border-border bg-muted/40 px-3 py-2">
        <input
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="Untitled group"
          className="min-w-0 flex-1 bg-transparent text-xs font-medium outline-none placeholder:text-muted-foreground"
        />
        <span className="shrink-0 text-[0.65rem] text-muted-foreground tabular-nums">
          {resolved.length} {resolved.length === 1 ? "trade" : "trades"} · net{" "}
          <span
            className={cn(
              "font-medium",
              net >= 0
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-rose-600 dark:text-rose-400",
            )}
          >
            {formatPnl(net, { precision: "cents" })}
          </span>
        </span>
      </div>

      <div className="divide-y divide-border/50">
        {visible.map(({ id, trade }) =>
          trade ? (
            <TradeTableRow
              key={id}
              trade={trade}
              onRemove={() => removeTrade(id)}
            />
          ) : (
            <MissingTradeRow key={id} onRemove={() => removeTrade(id)} />
          ),
        )}
      </div>

      {hidden > 0 || expanded ? (
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="w-full border-t border-border px-3 py-1.5 text-left text-[0.65rem] text-muted-foreground hover:bg-accent/50"
        >
          {expanded ? "Show less" : `Show ${hidden} more`}
        </button>
      ) : null}
    </div>
  );
}

export type { SerializedTradeTableNode };

/** Serialization lives in @tradstry/notebook-core; only rendering is here. */
export class TradeTableNode extends TradeTableSchema<ReactNode> {
  static clone(node: TradeTableNode): TradeTableNode {
    return new TradeTableNode(node.__tradeIds, node.__label, node.__key);
  }

  static importJSON(serialized: SerializedTradeTableNode): TradeTableNode {
    return new TradeTableNode(serialized.tradeIds, serialized.label);
  }

  createDOM(): HTMLElement {
    return document.createElement("div");
  }

  decorate(): ReactNode {
    return (
      <TradeTable
        nodeKey={this.getKey()}
        tradeIds={this.getTradeIds()}
        label={this.getLabel()}
      />
    );
  }
}

export function $createTradeTableNode(
  tradeIds: string[],
  label = "",
): TradeTableNode {
  return new TradeTableNode(tradeIds, label);
}

export function $isTradeTableNode(
  node: LexicalNode | null | undefined,
): node is TradeTableNode {
  return node instanceof TradeTableNode;
}
