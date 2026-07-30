import { DecoratorNode } from "lexical";
import type { NodeKey, SerializedLexicalNode, Spread } from "lexical";

/**
 * Schema-only node classes. They own the wire format — `getType`, `importJSON`,
 * `exportJSON` — and nothing else. `decorate()` is React, so it lives in the
 * clients: each subclasses one of these, overrides `decorate`/`clone`/`importJSON`,
 * and keeps the same `static getType()` string.
 *
 * The headless projector registers these base classes directly. That is the whole
 * point: one implementation of the wire format, not three.
 */
export abstract class SchemaDecoratorNode<T> extends DecoratorNode<T> {
  // Never reached headlessly (@lexical/headless skips reconciliation) and
  // overridden by every rendering client. `document` must not be referenced here.
  createDOM(): HTMLElement {
    return null as unknown as HTMLElement;
  }

  updateDOM(): false {
    return false;
  }

  decorate(): T {
    return null as unknown as T;
  }
}

export type SerializedNotebookImageNode = Spread<
  {
    type: "notebook-image";
    version: 2;
    hash: string;
    altText: string;
    width: number;
    height: number;
  },
  SerializedLexicalNode
>;

export class NotebookImageNode<T = unknown> extends SchemaDecoratorNode<T> {
  __hash: string;
  __altText: string;
  __width: number;
  __height: number;

  static getType(): string {
    return "notebook-image";
  }

  static clone(node: NotebookImageNode): NotebookImageNode {
    return new NotebookImageNode(
      node.__hash,
      node.__altText,
      node.__width,
      node.__height,
      node.__key,
    );
  }

  static importJSON(serialized: SerializedNotebookImageNode): NotebookImageNode {
    return new NotebookImageNode(
      serialized.hash,
      serialized.altText,
      serialized.width,
      serialized.height,
    );
  }

  constructor(
    hash: string,
    altText: string,
    width: number,
    height: number,
    key?: NodeKey,
  ) {
    super(key);
    this.__hash = hash;
    this.__altText = altText;
    this.__width = width;
    this.__height = height;
  }

  /** Block-level, matching the web editor. Inline would wrap it in a paragraph. */
  isInline(): false {
    return false;
  }

  exportJSON(): SerializedNotebookImageNode {
    return {
      ...super.exportJSON(),
      type: "notebook-image",
      version: 2,
      hash: this.__hash,
      altText: this.__altText,
      width: this.__width,
      height: this.__height,
    };
  }
}

export type SerializedNotebookVideoNode = Spread<
  {
    type: "notebook-video";
    version: 2;
    hash: string;
    altText: string;
  },
  SerializedLexicalNode
>;

export class NotebookVideoNode<T = unknown> extends SchemaDecoratorNode<T> {
  __hash: string;
  __altText: string;

  static getType(): string {
    return "notebook-video";
  }

  static clone(node: NotebookVideoNode): NotebookVideoNode {
    return new NotebookVideoNode(node.__hash, node.__altText, node.__key);
  }

  static importJSON(serialized: SerializedNotebookVideoNode): NotebookVideoNode {
    return new NotebookVideoNode(serialized.hash, serialized.altText);
  }

  constructor(hash: string, altText: string, key?: NodeKey) {
    super(key);
    this.__hash = hash;
    this.__altText = altText;
  }

  isInline(): false {
    return false;
  }

  exportJSON(): SerializedNotebookVideoNode {
    return {
      ...super.exportJSON(),
      type: "notebook-video",
      version: 2,
      hash: this.__hash,
      altText: this.__altText,
    };
  }
}

export type SerializedLinkedTradeNode = Spread<
  { type: "linked-trade"; version: 1; tradeId: string },
  SerializedLexicalNode
>;

export class LinkedTradeNode<T = unknown> extends SchemaDecoratorNode<T> {
  __tradeId: string;

  static getType(): string {
    return "linked-trade";
  }

  static clone(node: LinkedTradeNode): LinkedTradeNode {
    return new LinkedTradeNode(node.__tradeId, node.__key);
  }

  static importJSON(serialized: SerializedLinkedTradeNode): LinkedTradeNode {
    return new LinkedTradeNode(serialized.tradeId);
  }

  constructor(tradeId: string, key?: NodeKey) {
    super(key);
    this.__tradeId = tradeId;
  }

  isInline(): true {
    return true;
  }

  exportJSON(): SerializedLinkedTradeNode {
    return {
      ...super.exportJSON(),
      type: "linked-trade",
      version: 1,
      tradeId: this.__tradeId,
    };
  }
}

export type SerializedTradeTableNode = Spread<
  { type: "trade-table"; version: 1; tradeIds: string[]; label: string },
  SerializedLexicalNode
>;

/**
 * A group of linked trades rendered as one block. Like LinkedTradeNode it stores
 * only ids — the rows read live trades from the client, so P&L is never a snapshot
 * frozen at insert time. Membership is fixed; editing a trade updates its numbers.
 */
export class TradeTableNode<T = unknown> extends SchemaDecoratorNode<T> {
  __tradeIds: string[];
  __label: string;

  static getType(): string {
    return "trade-table";
  }

  static clone(node: TradeTableNode): TradeTableNode {
    return new TradeTableNode(node.__tradeIds, node.__label, node.__key);
  }

  static importJSON(serialized: SerializedTradeTableNode): TradeTableNode {
    return new TradeTableNode(serialized.tradeIds, serialized.label);
  }

  // @lexical/yjs materialises nodes from a Y type by calling the constructor with
  // no arguments, so both fields must survive being undefined.
  constructor(tradeIds: string[] = [], label = "", key?: NodeKey) {
    super(key);
    this.__tradeIds = [...(tradeIds ?? [])];
    this.__label = label ?? "";
  }

  isInline(): false {
    return false;
  }

  getTradeIds(): string[] {
    return [...this.__tradeIds];
  }

  setTradeIds(tradeIds: string[]): void {
    this.getWritable().__tradeIds = [...(tradeIds ?? [])];
  }

  getLabel(): string {
    return this.__label;
  }

  setLabel(label: string): void {
    this.getWritable().__label = label;
  }

  exportJSON(): SerializedTradeTableNode {
    return {
      ...super.exportJSON(),
      type: "trade-table",
      version: 1,
      tradeIds: [...this.__tradeIds],
      label: this.__label,
    };
  }
}

export type SerializedGhostTextNode = Spread<
  { type: "ghost-text"; version: 1; text: string },
  SerializedLexicalNode
>;

/**
 * Transient AI suggestion. The web strips it before persisting `document_json`,
 * but a CRDT syncs the whole editor state — so it reaches the projector and must
 * parse. `stripGhostText` removes it on the way to `document_json`.
 */
export class GhostTextNode<T = unknown> extends SchemaDecoratorNode<T> {
  __text: string;

  static getType(): string {
    return "ghost-text";
  }

  static clone(node: GhostTextNode): GhostTextNode {
    return new GhostTextNode(node.__text, node.__key);
  }

  static importJSON(serialized: SerializedGhostTextNode): GhostTextNode {
    return new GhostTextNode(serialized.text);
  }

  constructor(text: string, key?: NodeKey) {
    super(key);
    this.__text = text;
  }

  isInline(): true {
    return true;
  }

  exportJSON(): SerializedGhostTextNode {
    return {
      ...super.exportJSON(),
      type: "ghost-text",
      text: this.__text,
      version: 1,
    };
  }
}
