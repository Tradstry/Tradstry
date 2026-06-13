"use client";

import type { ReactElement } from "react";
import {
  $applyNodeReplacement,
  DecoratorNode,
  type DOMConversionMap,
  type DOMExportOutput,
  type EditorConfig,
  type LexicalNode,
  type NodeKey,
  type SerializedLexicalNode,
} from "lexical";

export type SerializedGhostTextNode = SerializedLexicalNode & {
  text: string;
};

export class GhostTextNode extends DecoratorNode<ReactElement> {
  __text: string;

  static getType(): string {
    return "ghost-text";
  }

  static clone(node: GhostTextNode): GhostTextNode {
    return new GhostTextNode(node.__text, node.__key);
  }

  constructor(text: string, key?: NodeKey) {
    super(key);
    this.__text = text;
  }

  createDOM(_config: EditorConfig): HTMLElement {
    const span = document.createElement("span");
    span.style.color = "var(--muted-foreground)";
    span.style.pointerEvents = "none";
    span.style.userSelect = "none";
    return span;
  }

  updateDOM(): false {
    return false;
  }

  static importJSON(serializedNode: SerializedGhostTextNode): GhostTextNode {
    return $createGhostTextNode(serializedNode.text);
  }

  exportJSON(): SerializedGhostTextNode {
    return {
      ...super.exportJSON(),
      type: "ghost-text",
      text: this.__text,
      version: 1,
    };
  }

  exportDOM(): DOMExportOutput {
    return { element: null };
  }

  static importDOM(): DOMConversionMap | null {
    return null;
  }

  getTextContent(): string {
    return "";
  }

  decorate(): ReactElement {
    return (
      <span
        style={{
          color: "var(--muted-foreground)",
          pointerEvents: "none",
          userSelect: "none",
        }}
      >
        {this.__text}
      </span>
    );
  }

  getText(): string {
    return this.__text;
  }
}

export function $createGhostTextNode(text: string): GhostTextNode {
  return $applyNodeReplacement(new GhostTextNode(text));
}

export function $isGhostTextNode(
  node: LexicalNode | null | undefined,
): node is GhostTextNode {
  return node instanceof GhostTextNode;
}
