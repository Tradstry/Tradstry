import {
  $createParagraphNode,
  $getSelection,
  $isRangeSelection,
  type ElementNode,
  type LexicalEditor,
} from "lexical";
import { $setBlocksType } from "@lexical/selection";
import {
  $createHeadingNode,
  $createQuoteNode,
  type HeadingTagType,
} from "@lexical/rich-text";
import { $createCodeNode } from "@lexical/code";
import {
  INSERT_CHECK_LIST_COMMAND,
  INSERT_ORDERED_LIST_COMMAND,
  INSERT_UNORDERED_LIST_COMMAND,
} from "@lexical/list";
import { TOGGLE_LINK_COMMAND } from "@lexical/link";
import { INSERT_HORIZONTAL_RULE_COMMAND } from "@lexical/react/LexicalHorizontalRuleNode";

// Block/list actions shared by the toolbar and the "/" slash menu, so the two
// entry points can't drift apart.

export const DEFAULT_CODE_LANGUAGE = "javascript";

export type BlockKey = "paragraph" | "h1" | "h2" | "h3" | "quote" | "code";
export type ListKey = "bullet" | "number" | "check";

function setBlocks(editor: LexicalEditor, create: () => ElementNode) {
  editor.update(() => {
    const selection = $getSelection();
    if ($isRangeSelection(selection)) $setBlocksType(selection, create);
  });
}

export function applyBlock(editor: LexicalEditor, key: BlockKey) {
  switch (key) {
    case "paragraph":
      return setBlocks(editor, () => $createParagraphNode());
    case "h1":
    case "h2":
    case "h3":
      return setBlocks(editor, () => $createHeadingNode(key as HeadingTagType));
    case "quote":
      return setBlocks(editor, () => $createQuoteNode());
    case "code":
      return setBlocks(editor, () => $createCodeNode(DEFAULT_CODE_LANGUAGE));
  }
}

export function insertList(editor: LexicalEditor, kind: ListKey) {
  const command =
    kind === "bullet"
      ? INSERT_UNORDERED_LIST_COMMAND
      : kind === "number"
        ? INSERT_ORDERED_LIST_COMMAND
        : INSERT_CHECK_LIST_COMMAND;
  editor.dispatchCommand(command, undefined);
}

export function insertDivider(editor: LexicalEditor) {
  editor.dispatchCommand(INSERT_HORIZONTAL_RULE_COMMAND, undefined);
}

export function promptLink(editor: LexicalEditor) {
  const url = window.prompt("Link URL");
  if (url) editor.dispatchCommand(TOGGLE_LINK_COMMAND, url);
}
