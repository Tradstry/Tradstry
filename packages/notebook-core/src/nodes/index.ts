import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { ListItemNode, ListNode } from "@lexical/list";
import { CodeHighlightNode, CodeNode } from "@lexical/code";
import { AutoLinkNode, LinkNode } from "@lexical/link";
import { HorizontalRuleNode } from "@lexical/react/LexicalHorizontalRuleNode";

import {
  GhostTextNode,
  LinkedTradeNode,
  NotebookImageNode,
  NotebookVideoNode,
} from "./custom";

/**
 * The UNION of every client's node types — the single source of truth.
 *
 * Both clients and the projector write and read one shared Y.Doc. A node type
 * registered by one side but not another throws on parse, and the projection is
 * then never written: the note's title and its embeddings freeze.
 *
 * A rendering client subclasses the custom nodes to supply `decorate()` and
 * passes its own subclass here instead of the schema class.
 */
export const STANDARD_NODES = [
  HeadingNode,
  QuoteNode,
  ListNode,
  ListItemNode,
  CodeNode,
  CodeHighlightNode,
  AutoLinkNode,
  LinkNode,
  HorizontalRuleNode,
] as const;

export const CUSTOM_SCHEMA_NODES = [
  NotebookImageNode,
  NotebookVideoNode,
  LinkedTradeNode,
  GhostTextNode,
] as const;

/** Headless registration: schema classes, no rendering. Used by the projector. */
export const NODES = [...STANDARD_NODES, ...CUSTOM_SCHEMA_NODES];

export * from "./custom";
