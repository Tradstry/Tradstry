"use client";

import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  LinkedTradeNode as LinkedTradeSchema,
  type SerializedLinkedTradeNode,
} from "@tradstry/notebook-core";
import { $getNodeByKey, type LexicalNode, type NodeKey } from "lexical";
import { createContext, type ReactNode, useCallback, useContext } from "react";
import type { JournalEntry } from "@tradstry/app-ui/lib/types/journal";
import { cn, formatPnl } from "@tradstry/app-ui/lib/utils";

// ---------------------------------------------------------------------------
// Context — provides the live trades list + unlink handler to every chip
// instance. Trades are kept out of the node's serialized JSON so that the
// chip always reflects the latest P&L / status from the journal query
// rather than a snapshot frozen at insert time.
// ---------------------------------------------------------------------------

type LinkedTradeContextValue = {
  trades: JournalEntry[];
  onUnlinkTrade?: (tradeId: string) => void;
};

/** Shared with TradeTableNode so both read the same live trades and unlink handler. */
export const LinkedTradeContext = createContext<LinkedTradeContextValue>({
  trades: [],
});

export function LinkedTradeProvider({
  children,
  trades,
  onUnlinkTrade,
}: {
  children: ReactNode;
  trades: JournalEntry[];
  onUnlinkTrade?: (tradeId: string) => void;
}) {
  return (
    <LinkedTradeContext.Provider value={{ trades, onUnlinkTrade }}>
      {children}
    </LinkedTradeContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Chip component
// ---------------------------------------------------------------------------

function LinkedTradeChip({
  nodeKey,
  tradeId,
}: {
  nodeKey: NodeKey;
  tradeId: string;
}) {
  const [editor] = useLexicalComposerContext();
  const { trades, onUnlinkTrade } = useContext(LinkedTradeContext);
  const trade = trades.find((t) => t.id === tradeId);

  const removeNode = useCallback(() => {
    editor.update(() => {
      const node = $getNodeByKey(nodeKey);
      if (node) {
        node.remove();
      }
    });
    if (onUnlinkTrade) {
      onUnlinkTrade(tradeId);
    }
  }, [editor, nodeKey, onUnlinkTrade, tradeId]);

  if (!trade) {
    // Trade was deleted from the journal but the pill is still in the doc.
    return (
      <span
        contentEditable={false}
        className="group mx-1 inline-flex items-center gap-1.5 rounded-md border border-border bg-accent px-2 py-0.5 align-middle text-[0.7rem] font-medium text-muted-foreground"
      >
        <span>Trade no longer available</span>
        <button
          type="button"
          onClick={removeNode}
          className="rounded p-0.5 text-muted-foreground opacity-0 transition-opacity hover:bg-muted hover:text-foreground group-hover:opacity-100"
          title="Unlink"
        >
          <HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
        </button>
      </span>
    );
  }

  const isProfit = trade.status === "profit";

  return (
    <span
      contentEditable={false}
      className={cn(
        "group mx-1 inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 align-middle text-[0.7rem] font-medium",
        isProfit
          ? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/50 dark:text-emerald-300"
          : "border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900 dark:bg-rose-950/50 dark:text-rose-300",
      )}
    >
      <span className="font-semibold tracking-wide">{trade.symbol}</span>
      <span className="tabular-nums">
        {formatPnl(trade.totalPl, { precision: "cents" })}
      </span>
      <button
        type="button"
        onClick={removeNode}
        className={cn(
          "rounded p-0.5 opacity-0 transition-opacity group-hover:opacity-100",
          isProfit
            ? "text-emerald-600 hover:bg-emerald-100 dark:text-emerald-400 dark:hover:bg-emerald-900/50"
            : "text-rose-600 hover:bg-rose-100 dark:text-rose-400 dark:hover:bg-rose-900/50",
        )}
        title="Unlink"
      >
        <HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
      </button>
    </span>
  );
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

export type { SerializedLinkedTradeNode };

/** Serialization lives in @tradstry/notebook-core; only rendering is here. */
export class LinkedTradeNode extends LinkedTradeSchema<ReactNode> {
  static clone(node: LinkedTradeNode): LinkedTradeNode {
    return new LinkedTradeNode(node.__tradeId, node.__key);
  }

  static importJSON(serialized: SerializedLinkedTradeNode): LinkedTradeNode {
    return $createLinkedTradeNode(serialized.tradeId);
  }

  getTradeId(): string {
    return this.__tradeId;
  }

  createDOM(): HTMLElement {
    return document.createElement("span");
  }

  decorate(): ReactNode {
    return <LinkedTradeChip nodeKey={this.getKey()} tradeId={this.__tradeId} />;
  }
}

export function $createLinkedTradeNode(tradeId: string): LinkedTradeNode {
  return new LinkedTradeNode(tradeId);
}

export function $isLinkedTradeNode(
  node: LexicalNode | null | undefined,
): node is LinkedTradeNode {
  return node instanceof LinkedTradeNode;
}
