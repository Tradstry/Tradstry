import type { JSX } from "react";
import {
  GhostTextNode as GhostTextSchema,
  LinkedTradeNode as LinkedTradeSchema,
  NotebookImageNode as NotebookImageSchema,
  NotebookVideoNode as NotebookVideoSchema,
  STANDARD_NODES,
  type SerializedGhostTextNode,
  type SerializedLinkedTradeNode,
  type SerializedNotebookImageNode,
  type SerializedNotebookVideoNode,
} from "@tradstry/notebook-core";

/**
 * The desktop's rendering half of the shared node schema. Serialization lives in
 * `@tradstry/notebook-core`; only `createDOM`/`decorate` are here.
 *
 * These render placeholders: the desktop notebook has no image, video, or
 * trade-chip UI yet. A note authored on the web still parses, round-trips, and
 * syncs — it just is not fully rendered here. Losing the node would be data loss;
 * showing a placeholder is not.
 */

const placeholder = (label: string) => (
  <span className="inline-flex items-center rounded-md border border-dashed border-zinc-300 bg-zinc-50 px-2 py-1 text-xs text-zinc-500 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400">
    {label}
  </span>
);

export class NotebookImageNode extends NotebookImageSchema<JSX.Element> {
  static clone(node: NotebookImageNode): NotebookImageNode {
    return new NotebookImageNode(
      node.__imageId,
      node.__src,
      node.__altText,
      node.__width,
      node.__height,
      node.__key,
    );
  }
  static importJSON(json: SerializedNotebookImageNode): NotebookImageNode {
    return new NotebookImageNode(
      json.imageId,
      json.src,
      json.altText,
      json.width,
      json.height,
    );
  }
  createDOM(): HTMLElement {
    return document.createElement("div");
  }
  decorate(): JSX.Element {
    return placeholder(this.__altText || "Image");
  }
}

export class NotebookVideoNode extends NotebookVideoSchema<JSX.Element> {
  static clone(node: NotebookVideoNode): NotebookVideoNode {
    return new NotebookVideoNode(node.__videoId, node.__src, node.__altText, node.__key);
  }
  static importJSON(json: SerializedNotebookVideoNode): NotebookVideoNode {
    return new NotebookVideoNode(json.videoId, json.src, json.altText);
  }
  createDOM(): HTMLElement {
    return document.createElement("div");
  }
  decorate(): JSX.Element {
    return placeholder(this.__altText || "Video");
  }
}

export class LinkedTradeNode extends LinkedTradeSchema<JSX.Element> {
  static clone(node: LinkedTradeNode): LinkedTradeNode {
    return new LinkedTradeNode(node.__tradeId, node.__key);
  }
  static importJSON(json: SerializedLinkedTradeNode): LinkedTradeNode {
    return new LinkedTradeNode(json.tradeId);
  }
  createDOM(): HTMLElement {
    return document.createElement("span");
  }
  decorate(): JSX.Element {
    return placeholder(`Trade ${this.__tradeId.slice(0, 8)}`);
  }
}

export class GhostTextNode extends GhostTextSchema<JSX.Element> {
  static clone(node: GhostTextNode): GhostTextNode {
    return new GhostTextNode(node.__text, node.__key);
  }
  static importJSON(json: SerializedGhostTextNode): GhostTextNode {
    return new GhostTextNode(json.text);
  }
  createDOM(): HTMLElement {
    return document.createElement("span");
  }
  decorate(): JSX.Element {
    return <span className="text-zinc-400 dark:text-zinc-600">{this.__text}</span>;
  }
}

/** The union again, but with this client's rendering subclasses substituted in. */
export const DESKTOP_NODES = [
  ...STANDARD_NODES,
  NotebookImageNode,
  NotebookVideoNode,
  LinkedTradeNode,
  GhostTextNode,
];
