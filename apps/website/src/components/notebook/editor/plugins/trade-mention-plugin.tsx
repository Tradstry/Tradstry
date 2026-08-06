"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $insertNodeToNearestRoot } from "@lexical/utils";
import { $getRoot, type LexicalNode } from "lexical";
import { useCallback, useState } from "react";
import { LinkTradesDialog } from "@/components/notebook/link-trades-dialog";
import type { JournalEntry } from "@/lib/types/journal";
import { $isLinkedTradeNode } from "../nodes/linked-trade-node";
import {
  $createTradeTableNode,
  $isTradeTableNode,
} from "../nodes/trade-table-node";
import { AtMentionPlugin } from "./at-mention-plugin";

/** Every trade id already referenced by a chip or a table in this document. */
function collectLinkedIds(root: ReturnType<typeof $getRoot>): string[] {
  const ids: string[] = [];
  const stack: LexicalNode[] = [...root.getChildren()];
  while (stack.length > 0) {
    const node = stack.pop();
    if (!node) continue;
    if ($isLinkedTradeNode(node)) ids.push(node.getTradeId());
    else if ($isTradeTableNode(node)) ids.push(...node.getTradeIds());
    if ("getChildren" in node && typeof node.getChildren === "function") {
      stack.push(...(node.getChildren() as LexicalNode[]));
    }
  }
  return ids;
}

/**
 * The `@` picker plus the escalation dialog it opens. They share state, so they
 * live together rather than as two siblings in the editor's plugin list.
 */
export function TradeMentionPlugin({
  trades = [],
  onLinkTrade,
}: {
  trades?: JournalEntry[];
  onLinkTrade?: (tradeId: string) => void;
}) {
  const [editor] = useLexicalComposerContext();
  const [open, setOpen] = useState(false);
  const [seedQuery, setSeedQuery] = useState("");
  const [linkedIds, setLinkedIds] = useState<string[]>([]);

  const browseAll = useCallback(
    (query: string) => {
      // Snapshot what's already linked so the dialog can tick and disable those
      // rows rather than letting you insert a duplicate.
      editor.getEditorState().read(() => {
        setLinkedIds(collectLinkedIds($getRoot()));
      });
      setSeedQuery(query);
      setOpen(true);
    },
    [editor],
  );

  const insert = useCallback(
    (tradeIds: string[]) => {
      editor.update(() => {
        $insertNodeToNearestRoot($createTradeTableNode(tradeIds));
      });
      for (const id of tradeIds) onLinkTrade?.(id);
    },
    [editor, onLinkTrade],
  );

  return (
    <>
      <AtMentionPlugin
        trades={trades}
        onLinkTrade={onLinkTrade}
        onBrowseAll={browseAll}
      />
      <LinkTradesDialog
        open={open}
        onOpenChange={setOpen}
        trades={trades}
        initialQuery={seedQuery}
        alreadyLinkedIds={linkedIds}
        onInsert={insert}
      />
    </>
  );
}
