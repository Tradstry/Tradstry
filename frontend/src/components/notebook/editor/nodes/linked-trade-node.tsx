"use client";

import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getNodeByKey,
  DecoratorNode,
  type LexicalNode,
  type NodeKey,
  type SerializedLexicalNode,
  type Spread,
} from "lexical";
import { createContext, type ReactNode, useCallback, useContext } from "react";
import type { JournalEntry } from "@/lib/types/journal";
import { cn, formatPnl } from "@/lib/utils";

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

const LinkedTradeContext = createContext<LinkedTradeContextValue>({
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
        className="group mx-1 inline-flex items-center gap-1.5 rounded-md border border-slate-200 bg-slate-50 px-2 py-0.5 align-middle text-[0.7rem] font-medium text-slate-500"
      >
        <span>Trade no longer available</span>
        <button
          type="button"
          onClick={removeNode}
          className="rounded p-0.5 text-slate-400 opacity-0 transition-opacity hover:bg-slate-200 hover:text-slate-700 group-hover:opacity-100"
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
          ? "border-emerald-200 bg-emerald-50 text-emerald-700"
          : "border-rose-200 bg-rose-50 text-rose-700",
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
            ? "text-emerald-600 hover:bg-emerald-100"
            : "text-rose-600 hover:bg-rose-100",
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

export type SerializedLinkedTradeNode = Spread<
  {
    type: "linked-trade";
    version: 1;
    tradeId: string;
  },
  SerializedLexicalNode
>;

export class LinkedTradeNode extends DecoratorNode<JSX.Element> {
  __tradeId: string;

  static getType(): string {
    return "linked-trade";
  }

  static clone(node: LinkedTradeNode): LinkedTradeNode {
    return new LinkedTradeNode(node.__tradeId, node.__key);
  }

  static importJSON(serialized: SerializedLinkedTradeNode): LinkedTradeNode {
    return $createLinkedTradeNode(serialized.tradeId);
  }

  constructor(tradeId: string, key?: NodeKey) {
    super(key);
    this.__tradeId = tradeId;
  }

  exportJSON(): SerializedLinkedTradeNode {
    return {
      ...super.exportJSON(),
      type: "linked-trade",
      version: 1,
      tradeId: this.__tradeId,
    };
  }

  getTradeId(): string {
    return this.__tradeId;
  }

  createDOM(): HTMLElement {
    return document.createElement("span");
  }

  updateDOM(): false {
    return false;
  }

  isInline(): true {
    return true;
  }

  decorate(): JSX.Element {
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
