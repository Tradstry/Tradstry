"use client";

import {
  ArrowExpandDiagonal01Icon,
  Cancel01Icon,
  Delete02Icon,
  Download04Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  NotebookImageNode as NotebookImageSchema,
  type SerializedNotebookImageNode,
} from "@tradstry/notebook-core";
import { $getNodeByKey, type LexicalNode, type NodeKey } from "lexical";
import {
  createContext,
  type JSX,
  type ReactNode,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@tradstry/app-ui/components/ui/tooltip";
import type { NotebookImage } from "@tradstry/app-ui/lib/types/notebook";
import { getLocalBlob, revokeLocalBlob } from "../media-registry";

/** Four draggable corner grips. signX/signY map a pointer delta to growth. */
const RESIZE_CORNERS = [
  {
    key: "tl",
    signX: -1,
    signY: -1,
    pos: "top-0 left-0 -translate-x-1/2 -translate-y-1/2 cursor-nwse-resize",
  },
  {
    key: "tr",
    signX: 1,
    signY: -1,
    pos: "top-0 right-0 translate-x-1/2 -translate-y-1/2 cursor-nesw-resize",
  },
  {
    key: "bl",
    signX: -1,
    signY: 1,
    pos: "bottom-0 left-0 -translate-x-1/2 translate-y-1/2 cursor-nesw-resize",
  },
  {
    key: "br",
    signX: 1,
    signY: 1,
    pos: "bottom-0 right-0 translate-x-1/2 translate-y-1/2 cursor-nwse-resize",
  },
] as const;

type ResizeCorner = (typeof RESIZE_CORNERS)[number];

export type { SerializedNotebookImageNode };

const NotebookImageActionsContext = createContext<{
  onDeleteImage?: (hash: string) => Promise<void>;
  urlFor?: (hash: string) => string | undefined;
}>({});

export function useNotebookMediaActions() {
  return useContext(NotebookImageActionsContext);
}

/**
 * A media node stores only its content hash; the presigned `src` baked into it
 * expires. The body is a CRDT, so nothing rewrites that URL on load the way
 * `mergeNotebookImagesIntoDocumentJson` used to. Resolve it here, at render, from
 * the live image list — the same shape `LinkedTradeProvider` uses for trades.
 */
export function NotebookImageActionsProvider({
  children,
  images = [],
  onDeleteImage,
}: {
  children: ReactNode;
  images?: NotebookImage[];
  onDeleteImage?: (hash: string) => Promise<void>;
}) {
  const value = useMemo(() => {
    const byId = new Map(
      images.map((image) => [image.contentHash, image.secureUrl]),
    );
    return { onDeleteImage, urlFor: (hash: string) => byId.get(hash) };
  }, [images, onDeleteImage]);

  return (
    <NotebookImageActionsContext.Provider value={value}>
      {children}
    </NotebookImageActionsContext.Provider>
  );
}

function NotebookImageComponent({
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
  const { onDeleteImage, urlFor } = useContext(NotebookImageActionsContext);
  // A local blob: URL mid-upload takes precedence over the (possibly not-yet-
  // resolvable) server URL.
  const src = getLocalBlob(hash) ?? urlFor?.(hash);
  const [isExpanded, setIsExpanded] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [draftSize, setDraftSize] = useState<{
    width: number;
    height: number;
  } | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const resizeStateRef = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    startWidth: number;
    startHeight: number;
    aspectRatio: number;
    maxWidth: number;
    signX: number;
    signY: number;
    nextWidth: number;
    nextHeight: number;
  } | null>(null);
  const isTemp = Boolean(src?.startsWith("blob:"));
  const isPending = !src;
  const displayWidth = draftSize?.width ?? (width > 0 ? width : 0);
  const displayHeight = draftSize?.height ?? (height > 0 ? height : 0);

  const removeNode = useCallback(() => {
    editor.update(() => {
      const liveNode = $getNodeByKey(nodeKey);
      if (!liveNode || !$isNotebookImageNode(liveNode)) {
        return;
      }

      liveNode.remove();
    });
  }, [editor, nodeKey]);

  const updateNodeSize = useCallback(
    (nextWidth: number, nextHeight: number) => {
      editor.update(() => {
        const liveNode = $getNodeByKey(nodeKey);
        if (!liveNode || !$isNotebookImageNode(liveNode)) {
          return;
        }

        const writable = liveNode.getWritable();
        writable.__width = nextWidth;
        writable.__height = nextHeight;
      });
    },
    [editor, nodeKey],
  );

  const handleDelete = useCallback(async () => {
    if (isDeleting) {
      return;
    }

    if (getLocalBlob(hash) !== undefined) {
      // No resolved server copy yet — nothing to delete server-side.
      revokeLocalBlob(hash);
      removeNode();
      return;
    }

    if (!onDeleteImage) {
      removeNode();
      return;
    }

    try {
      setIsDeleting(true);
      await onDeleteImage(hash);
      removeNode();
    } catch (error) {
      console.error("Failed to delete notebook image", error);
    } finally {
      setIsDeleting(false);
    }
  }, [hash, isDeleting, onDeleteImage, removeNode]);

  const handleDownload = useCallback(() => {
    if (!src) {
      return;
    }
    const link = document.createElement("a");
    link.href = src;
    link.download = altText || "notebook-image";
    link.rel = "noopener";
    document.body.appendChild(link);
    link.click();
    link.remove();
  }, [altText, src]);

  const handleResizeMove = useCallback((event: PointerEvent) => {
    const resizeState = resizeStateRef.current;
    if (!resizeState) {
      return;
    }

    const deltaX = (event.clientX - resizeState.startX) * resizeState.signX;
    const deltaY = (event.clientY - resizeState.startY) * resizeState.signY;
    const nextWidthFromX = resizeState.startWidth + deltaX;
    const nextWidthFromY =
      (resizeState.startHeight + deltaY) * resizeState.aspectRatio;
    const nextWidth = Math.min(
      resizeState.maxWidth,
      Math.max(160, Math.max(nextWidthFromX, nextWidthFromY)),
    );
    const nextHeight = Math.max(120, nextWidth / resizeState.aspectRatio);

    resizeState.nextWidth = Math.round(nextWidth);
    resizeState.nextHeight = Math.round(nextHeight);

    setDraftSize({
      width: resizeState.nextWidth,
      height: resizeState.nextHeight,
    });
  }, []);

  const handleResizeEnd = useCallback(() => {
    const resizeState = resizeStateRef.current;
    if (!resizeState) {
      return;
    }

    window.removeEventListener("pointermove", handleResizeMove);
    window.removeEventListener("pointerup", handleResizeEnd);
    window.removeEventListener("pointercancel", handleResizeEnd);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";

    resizeStateRef.current = null;
    setIsResizing(false);
    setDraftSize(null);
    updateNodeSize(resizeState.nextWidth, resizeState.nextHeight);
  }, [handleResizeMove, updateNodeSize]);

  const handleResizeStart = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>, corner: ResizeCorner) => {
      event.preventDefault();
      event.stopPropagation();

      const image = imageRef.current;
      const container = containerRef.current;
      if (!image || !container) {
        return;
      }

      const imageRect = image.getBoundingClientRect();
      const containerRect = container.getBoundingClientRect();
      const aspectRatio =
        imageRect.width > 0 && imageRect.height > 0
          ? imageRect.width / imageRect.height
          : width > 0 && height > 0
            ? width / height
            : 1;

      resizeStateRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        startWidth: imageRect.width,
        startHeight: imageRect.height,
        aspectRatio,
        maxWidth: Math.max(160, containerRect.width),
        signX: corner.signX,
        signY: corner.signY,
        nextWidth: Math.round(imageRect.width),
        nextHeight: Math.round(imageRect.height),
      };

      setIsResizing(true);
      setDraftSize({
        width: Math.round(imageRect.width),
        height: Math.round(imageRect.height),
      });

      // Opposite-sign corners share a diagonal cursor.
      document.body.style.cursor =
        corner.signX * corner.signY > 0 ? "nwse-resize" : "nesw-resize";
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

  useEffect(() => {
    if (!isExpanded) {
      return;
    }

    const previousOverflow = document.body.style.overflow;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsExpanded(false);
      }
    };

    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [isExpanded]);

  return (
    <>
      <div className="group/notebook-image my-4 w-full" ref={containerRef}>
        <div className="relative inline-block max-w-full">
          <TooltipProvider>
            <div
              className={`absolute top-3 right-3 z-20 flex items-center gap-1.5 transition-opacity ${
                isTemp
                  ? "opacity-100"
                  : "opacity-0 group-hover/notebook-image:opacity-100 group-focus-within/notebook-image:opacity-100"
              }`}
            >
              {!isTemp ? (
                <>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label="Download image"
                        onClick={(event) => {
                          event.preventDefault();
                          event.stopPropagation();
                          handleDownload();
                        }}
                        className="rounded-lg bg-black/50 text-white hover:bg-black/70 hover:text-white"
                      >
                        <HugeiconsIcon icon={Download04Icon} strokeWidth={2} />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent side="bottom" sideOffset={6}>
                      Download
                    </TooltipContent>
                  </Tooltip>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label="Expand image"
                        onClick={(event) => {
                          event.preventDefault();
                          event.stopPropagation();
                          setIsExpanded(true);
                        }}
                        className="rounded-lg bg-black/50 text-white hover:bg-black/70 hover:text-white"
                      >
                        <HugeiconsIcon
                          icon={ArrowExpandDiagonal01Icon}
                          strokeWidth={2}
                        />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent side="bottom" sideOffset={6}>
                      Expand
                    </TooltipContent>
                  </Tooltip>
                </>
              ) : null}
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={isTemp ? "Cancel upload" : "Delete image"}
                    disabled={isDeleting}
                    onClick={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      void handleDelete();
                    }}
                    className="rounded-lg bg-black/50 text-white hover:bg-black/70 hover:text-white"
                  >
                    <HugeiconsIcon
                      icon={isTemp ? Cancel01Icon : Delete02Icon}
                      strokeWidth={2}
                    />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" sideOffset={6}>
                  {isTemp ? "Cancel upload" : "Delete"}
                </TooltipContent>
              </Tooltip>
            </div>
          </TooltipProvider>

          {isTemp || isPending ? (
            <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-xl bg-black/20">
              <div className="size-7 animate-spin rounded-full border-2 border-white/40 border-t-white" />
            </div>
          ) : null}

          <img
            ref={imageRef}
            src={src ?? ""}
            alt={altText}
            width={displayWidth || undefined}
            height={displayHeight || undefined}
            loading={isTemp ? "eager" : "lazy"}
            draggable={false}
            className="block h-auto max-w-full"
            style={{
              width: displayWidth > 0 ? `${displayWidth}px` : "100%",
            }}
          />

          {!isTemp ? (
            <div
              className={`pointer-events-none absolute inset-0 z-10 transition-opacity ${
                isResizing
                  ? "opacity-100"
                  : "opacity-0 group-hover/notebook-image:opacity-100 group-focus-within/notebook-image:opacity-100"
              }`}
            >
              <div className="absolute inset-0 rounded-sm ring-2 ring-blue-500" />
              {RESIZE_CORNERS.map((corner) => (
                <button
                  key={corner.key}
                  type="button"
                  aria-label="Resize image"
                  onPointerDown={(event) => handleResizeStart(event, corner)}
                  style={{ touchAction: "none" }}
                  className={`pointer-events-auto absolute ${corner.pos} size-3 rounded-[3px] border border-white bg-blue-500 shadow-sm`}
                />
              ))}
            </div>
          ) : null}
        </div>
      </div>

      {isExpanded
        ? createPortal(
            <div
              role="dialog"
              aria-modal="true"
              aria-label="Expanded image"
              className="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 p-6"
              onClick={(event) => {
                if (event.target === event.currentTarget) {
                  setIsExpanded(false);
                }
              }}
            >
              <Button
                autoFocus
                type="button"
                variant="ghost"
                size="icon"
                className="absolute top-4 right-4 z-20 rounded-full bg-black/70 text-white shadow-lg ring-1 ring-white/30 hover:bg-black/90 hover:text-white"
                aria-label="Close expanded image"
                onClick={() => setIsExpanded(false)}
              >
                <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2.3} />
              </Button>
              <img
                src={src ?? ""}
                alt={altText}
                className="max-h-[90vh] max-w-[90vw] object-contain"
              />
            </div>,
            document.body,
          )
        : null}
    </>
  );
}

/** Serialization lives in @tradstry/notebook-core; only rendering is here. */
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

  static importJSON(
    serializedNode: SerializedNotebookImageNode,
  ): NotebookImageNode {
    return $createNotebookImageNode({
      hash: serializedNode.hash,
      altText: serializedNode.altText,
      width: serializedNode.width,
      height: serializedNode.height,
    });
  }

  createDOM(): HTMLElement {
    return document.createElement("div");
  }

  decorate(): JSX.Element {
    return (
      <NotebookImageComponent
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
