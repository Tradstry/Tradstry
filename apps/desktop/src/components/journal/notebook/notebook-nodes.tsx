import type { JSX, PointerEvent as ReactPointerEvent } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $getNodeByKey, type LexicalNode, type NodeKey } from "lexical";
import {
  ArrowExpandDiagonal01Icon,
  Cancel01Icon,
  Delete02Icon,
  Download04Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { toast } from "sonner";
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
  type SerializedTradeTableNode,
  TradeTableNode as TradeTableSchema,
} from "@tradstry/notebook-core";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { deleteMedia, saveMedia } from "../../../backend";
import { useMediaResolver } from "./media-resolver";

const RESIZE_CORNERS = [
  { key: "nw", signX: -1, signY: -1, pos: "top-0 left-0 -translate-x-1/2 -translate-y-1/2 cursor-nwse-resize" },
  { key: "ne", signX: 1, signY: -1, pos: "top-0 right-0 translate-x-1/2 -translate-y-1/2 cursor-nesw-resize" },
  { key: "sw", signX: -1, signY: 1, pos: "bottom-0 left-0 -translate-x-1/2 translate-y-1/2 cursor-nesw-resize" },
  { key: "se", signX: 1, signY: 1, pos: "bottom-0 right-0 translate-x-1/2 translate-y-1/2 cursor-nwse-resize" },
] as const;

type ResizeCorner = (typeof RESIZE_CORNERS)[number];

/**
 * The desktop's rendering half of the shared node schema. Serialization lives in
 * `@tradstry/notebook-core`; only `createDOM`/`decorate` are here.
 *
 * Image and video nodes resolve their content hash to a local file path via
 * `useMediaResolver` (backed by the native media store); trade chips still render
 * a placeholder since the desktop has no trade-chip UI yet.
 */

const placeholder = (label: string) => (
  <span className="inline-flex items-center rounded-md border border-dashed border-zinc-300 bg-zinc-50 px-2 py-1 text-xs text-zinc-500 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400">
    {label}
  </span>
);

function SyncingBox({ percent }: { percent?: number }) {
  return (
    <div className="flex h-32 w-full max-w-sm flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-zinc-300 bg-zinc-50 text-xs text-zinc-500 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-400">
      <div className="flex items-center gap-2">
        <span className="size-3 animate-spin rounded-full border-2 border-zinc-300 border-t-zinc-500 dark:border-zinc-700 dark:border-t-zinc-400" />
        {percent === undefined
          ? "Syncing media…"
          : `Syncing media… ${percent}%`}
      </div>
      {percent !== undefined ? (
        <div className="h-1 w-40 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
          <div
            className="h-full rounded-full bg-zinc-400 transition-[width] duration-150 dark:bg-zinc-500"
            style={{ width: `${percent}%` }}
          />
        </div>
      ) : null}
    </div>
  );
}

function DesktopImage({
  nodeKey,
  hash,
  altText,
  width,
  height,
}: {
  nodeKey: NodeKey;
  hash: string;
  altText: string;
  width: number;
  height: number;
}) {
  const [editor] = useLexicalComposerContext();
  const { resolve } = useMediaResolver();
  const { src, thumb, pending, percent } = resolve(hash);
  const [isExpanded, setIsExpanded] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [draftSize, setDraftSize] = useState<{ width: number; height: number } | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const resizeStateRef = useRef<{
    startX: number;
    startY: number;
    aspectRatio: number;
    maxWidth: number;
    signX: number;
    signY: number;
    startWidth: number;
    startHeight: number;
    nextWidth: number;
    nextHeight: number;
  } | null>(null);

  const shown = src ?? thumb;
  const displayWidth = draftSize?.width ?? (width > 0 ? width : 0);

  const removeNode = useCallback(() => {
    editor.update(() => {
      const liveNode = $getNodeByKey(nodeKey);
      if ($isNotebookImageNode(liveNode)) {
        liveNode.remove();
      }
    });
  }, [editor, nodeKey]);

  const updateNodeSize = useCallback(
    (nextWidth: number, nextHeight: number) => {
      editor.update(() => {
        const liveNode = $getNodeByKey(nodeKey);
        if ($isNotebookImageNode(liveNode)) {
          const writable = liveNode.getWritable();
          writable.__width = nextWidth;
          writable.__height = nextHeight;
        }
      });
    },
    [editor, nodeKey],
  );

  const handleDelete = useCallback(async () => {
    if (isDeleting) return;
    try {
      setIsDeleting(true);
      await deleteMedia(hash);
      removeNode();
    } catch (error) {
      console.error("Failed to delete media", error);
      toast.error("Failed to delete image.");
    } finally {
      setIsDeleting(false);
    }
  }, [hash, isDeleting, removeNode]);

  const handleDownload = useCallback(async () => {
    try {
      await saveMedia(hash, altText || "notebook-image");
      toast.success("Saved to Downloads.");
    } catch (error) {
      console.error("Failed to save media", error);
      toast.error("Failed to save image.");
    }
  }, [altText, hash]);

  const handleResizeMove = useCallback((event: PointerEvent) => {
    const s = resizeStateRef.current;
    if (!s) return;
    const deltaX = (event.clientX - s.startX) * s.signX;
    const deltaY = (event.clientY - s.startY) * s.signY;
    const fromX = s.startWidth + deltaX;
    const fromY = (s.startHeight + deltaY) * s.aspectRatio;
    const nextWidth = Math.min(s.maxWidth, Math.max(160, Math.max(fromX, fromY)));
    const nextHeight = Math.max(120, nextWidth / s.aspectRatio);
    s.nextWidth = Math.round(nextWidth);
    s.nextHeight = Math.round(nextHeight);
    setDraftSize({ width: s.nextWidth, height: s.nextHeight });
  }, []);

  const handleResizeEnd = useCallback(() => {
    const s = resizeStateRef.current;
    if (!s) return;
    window.removeEventListener("pointermove", handleResizeMove);
    window.removeEventListener("pointerup", handleResizeEnd);
    window.removeEventListener("pointercancel", handleResizeEnd);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    resizeStateRef.current = null;
    setIsResizing(false);
    setDraftSize(null);
    updateNodeSize(s.nextWidth, s.nextHeight);
  }, [handleResizeMove, updateNodeSize]);

  const handleResizeStart = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>, corner: ResizeCorner) => {
      event.preventDefault();
      event.stopPropagation();
      const image = imageRef.current;
      const container = containerRef.current;
      if (!image || !container) return;
      const imageRect = image.getBoundingClientRect();
      const containerRect = container.getBoundingClientRect();
      const aspectRatio =
        imageRect.width > 0 && imageRect.height > 0
          ? imageRect.width / imageRect.height
          : width > 0 && height > 0
            ? width / height
            : 1;
      resizeStateRef.current = {
        startX: event.clientX,
        startY: event.clientY,
        aspectRatio,
        maxWidth: Math.max(160, containerRect.width),
        signX: corner.signX,
        signY: corner.signY,
        startWidth: imageRect.width,
        startHeight: imageRect.height,
        nextWidth: Math.round(imageRect.width),
        nextHeight: Math.round(imageRect.height),
      };
      setIsResizing(true);
      setDraftSize({ width: Math.round(imageRect.width), height: Math.round(imageRect.height) });
      document.body.style.cursor = corner.signX * corner.signY > 0 ? "nwse-resize" : "nesw-resize";
      document.body.style.userSelect = "none";
      window.addEventListener("pointermove", handleResizeMove);
      window.addEventListener("pointerup", handleResizeEnd);
      window.addEventListener("pointercancel", handleResizeEnd);
    },
    [handleResizeEnd, handleResizeMove, height, width],
  );

  useEffect(() => {
    return () => {
      window.removeEventListener("pointermove", handleResizeMove);
      window.removeEventListener("pointerup", handleResizeEnd);
      window.removeEventListener("pointercancel", handleResizeEnd);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
  }, [handleResizeEnd, handleResizeMove]);

  if (pending && !shown) {
    return <SyncingBox percent={percent} />;
  }

  return (
    <>
      <div className="group/nb-image my-4 w-full" ref={containerRef}>
        <div className="relative inline-block max-w-full">
          <TooltipProvider>
            <div className="absolute top-3 right-3 z-20 flex items-center gap-1.5 opacity-0 transition-opacity group-hover/nb-image:opacity-100 group-focus-within/nb-image:opacity-100">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label="Download image"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      void handleDownload();
                    }}
                    className="size-8 rounded-lg bg-black/50 text-white hover:bg-black/70 hover:text-white"
                  >
                    <HugeiconsIcon icon={Download04Icon} strokeWidth={2} />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" sideOffset={6}>Download</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label="Expand image"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setIsExpanded(true);
                    }}
                    className="size-8 rounded-lg bg-black/50 text-white hover:bg-black/70 hover:text-white"
                  >
                    <HugeiconsIcon icon={ArrowExpandDiagonal01Icon} strokeWidth={2} />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" sideOffset={6}>Expand</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label="Delete image"
                    disabled={isDeleting}
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      void handleDelete();
                    }}
                    className="size-8 rounded-lg bg-black/50 text-white hover:bg-black/70 hover:text-white"
                  >
                    <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" sideOffset={6}>Delete</TooltipContent>
              </Tooltip>
            </div>
          </TooltipProvider>

          <img
            ref={imageRef}
            src={shown}
            alt={altText}
            draggable={false}
            className="block h-auto max-w-full rounded-lg"
            style={{ width: displayWidth > 0 ? `${displayWidth}px` : "100%" }}
          />

          <div
            className={`pointer-events-none absolute inset-0 z-10 transition-opacity ${
              isResizing
                ? "opacity-100"
                : "opacity-0 group-hover/nb-image:opacity-100 group-focus-within/nb-image:opacity-100"
            }`}
          >
            <div className="absolute inset-0 rounded-sm ring-2 ring-blue-500" />
            {RESIZE_CORNERS.map((corner) => (
              <button
                key={corner.key}
                type="button"
                aria-label="Resize image"
                onPointerDown={(e) => handleResizeStart(e, corner)}
                style={{ touchAction: "none" }}
                className={`pointer-events-auto absolute ${corner.pos} size-3 rounded-[3px] border border-white bg-blue-500 shadow-sm`}
              />
            ))}
          </div>
        </div>
      </div>

      {isExpanded ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-6"
          onClick={(e) => {
            if (e.target === e.currentTarget) setIsExpanded(false);
          }}
        >
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="absolute top-4 right-4 rounded-full bg-white/10 text-white hover:bg-white/20 hover:text-white"
            aria-label="Close expanded image"
            onClick={() => setIsExpanded(false)}
          >
            <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2.3} />
          </Button>
          <img src={src ?? shown} alt={altText} className="max-h-[90vh] max-w-[90vw] object-contain" />
        </div>
      ) : null}
    </>
  );
}

function DesktopVideo({ hash }: { hash: string }) {
  const { resolve } = useMediaResolver();
  const { src, thumb, pending, percent } = resolve(hash);

  if (pending && !src && !thumb) {
    return <SyncingBox percent={percent} />;
  }

  return (
    // biome-ignore lint/a11y/useMediaCaption: pasted clips have no captions
    <video
      src={src}
      poster={thumb}
      controls
      preload="metadata"
      className="max-h-[32rem] max-w-full rounded-lg"
    />
  );
}

export class NotebookImageNode extends NotebookImageSchema<JSX.Element> {
  static clone(node: NotebookImageNode): NotebookImageNode {
    return new NotebookImageNode(
      node.__hash,
      node.__altText,
      node.__width,
      node.__height,
      node.__key,
    );
  }
  static importJSON(json: SerializedNotebookImageNode): NotebookImageNode {
    return new NotebookImageNode(json.hash, json.altText, json.width, json.height);
  }
  createDOM(): HTMLElement {
    return document.createElement("div");
  }
  decorate(): JSX.Element {
    return (
      <DesktopImage
        nodeKey={this.getKey()}
        hash={this.__hash}
        altText={this.__altText}
        width={this.__width}
        height={this.__height}
      />
    );
  }
}

export function $createNotebookImageNode({
  hash,
  altText = "",
  width = 0,
  height = 0,
}: {
  hash: string;
  altText?: string;
  width?: number;
  height?: number;
}): NotebookImageNode {
  return new NotebookImageNode(hash, altText, width, height);
}

export function $isNotebookImageNode(
  node: LexicalNode | null | undefined,
): node is NotebookImageNode {
  return node instanceof NotebookImageNode;
}

export class NotebookVideoNode extends NotebookVideoSchema<JSX.Element> {
  static clone(node: NotebookVideoNode): NotebookVideoNode {
    return new NotebookVideoNode(node.__hash, node.__altText, node.__key);
  }
  static importJSON(json: SerializedNotebookVideoNode): NotebookVideoNode {
    return new NotebookVideoNode(json.hash, json.altText);
  }
  createDOM(): HTMLElement {
    return document.createElement("div");
  }
  decorate(): JSX.Element {
    return <DesktopVideo hash={this.__hash} />;
  }
}

export function $createNotebookVideoNode({
  hash,
  altText = "",
}: {
  hash: string;
  altText?: string;
}): NotebookVideoNode {
  return new NotebookVideoNode(hash, altText);
}

export function $isNotebookVideoNode(
  node: LexicalNode | null | undefined,
): node is NotebookVideoNode {
  return node instanceof NotebookVideoNode;
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

export class TradeTableNode extends TradeTableSchema<JSX.Element> {
  static clone(node: TradeTableNode): TradeTableNode {
    return new TradeTableNode(node.__tradeIds, node.__label, node.__key);
  }
  static importJSON(json: SerializedTradeTableNode): TradeTableNode {
    return new TradeTableNode(json.tradeIds, json.label);
  }
  createDOM(): HTMLElement {
    return document.createElement("div");
  }
  decorate(): JSX.Element {
    const count = this.__tradeIds.length;
    return placeholder(`${count} linked ${count === 1 ? "trade" : "trades"}`);
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
  TradeTableNode,
  GhostTextNode,
];
