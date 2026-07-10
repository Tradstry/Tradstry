"use client";

import type { ReactElement } from "react";
import {
  $applyNodeReplacement,
  type DOMConversionMap,
  type DOMExportOutput,
  type LexicalNode,
} from "lexical";
import {
  GhostTextNode as GhostTextSchema,
  type SerializedGhostTextNode,
} from "@tradstry/notebook-core";

export type { SerializedGhostTextNode };

/** Serialization lives in @tradstry/notebook-core; only rendering is here. */
export class GhostTextNode extends GhostTextSchema<ReactElement> {
  static clone(node: GhostTextNode): GhostTextNode {
    return new GhostTextNode(node.__text, node.__key);
  }

  static importJSON(serializedNode: SerializedGhostTextNode): GhostTextNode {
    return $createGhostTextNode(serializedNode.text);
  }

  createDOM(): HTMLElement {
    const span = document.createElement("span");
    span.style.color = "var(--muted-foreground)";
    span.style.pointerEvents = "none";
    span.style.userSelect = "none";
    return span;
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
