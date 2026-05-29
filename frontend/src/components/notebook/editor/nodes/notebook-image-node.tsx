"use client";

import {
  ArrowExpandDiagonal01Icon,
  Cancel01Icon,
  Delete02Icon,
  Download04Icon,
  Resize01Icon,
} from "@hugeicons/core-free-icons";
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
import {
  createContext,
  type JSX,
  type ReactNode,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { abortUpload } from "../upload-registry";

export type SerializedNotebookImageNode = Spread<
  {
    type: "notebook-image";
    version: 1;
    imageId: string;
    src: string;
    altText: string;
    width: number;
    height: number;
  },
  SerializedLexicalNode
>;

const NotebookImageActionsContext = createContext<{
  onDeleteImage?: (imageId: string) => Promise<void>;
}>({});

export function useNotebookMediaActions() {
  return useContext(NotebookImageActionsContext);
}

export function NotebookImageActionsProvider({
  children,
  onDeleteImage,
}: {
  children: ReactNode;
  onDeleteImage?: (imageId: string) => Promise<void>;
}) {
  return (
    <NotebookImageActionsContext.Provider value={{ onDeleteImage }}>
      {children}
    </NotebookImageActionsContext.Provider>
  );
}

function NotebookImageComponent({
  nodeKey,
  imageId,
  src,
  altText,
  width,
  height,
}: {
  nodeKey: NodeKey;
  imageId: string;
  src: string;
  altText: string;
  width: number;
  height: number;
}) {
  const [editor] = useLexicalComposerContext();
  const { onDeleteImage } = useContext(NotebookImageActionsContext);
  const [isExpanded, setIsExpanded] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isResizeMode, setIsResizeMode] = useState(false);
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
    nextWidth: number;
    nextHeight: number;
  } | null>(null);
  const isTemp = src?.startsWith("blob:") ?? false;
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

    if (imageId.startsWith("local-")) {
      // Still uploading — abort the in-flight request, then drop the node.
      abortUpload(imageId);
      removeNode();
      return;
    }

    if (!onDeleteImage) {
      removeNode();
      return;
    }

    try {
      setIsDeleting(true);
      await onDeleteImage(imageId);
      removeNode();
    } catch (error) {
      console.error("Failed to delete notebook image", error);
    } finally {
      setIsDeleting(false);
    }
  }, [imageId, isDeleting, onDeleteImage, removeNode]);

  const handleDownload = useCallback(() => {
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

    const deltaX = event.clientX - resizeState.startX;
    const deltaY = event.clientY - resizeState.startY;
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
    (event: ReactPointerEvent<HTMLButtonElement>) => {
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
        nextWidth: Math.round(imageRect.width),
        nextHeight: Math.round(imageRect.height),
      };

      setIsResizeMode(true);
      setIsResizing(true);
      setDraftSize({
        width: Math.round(imageRect.width),
        height: Math.round(imageRect.height),
      });

      document.body.style.cursor = "se-resize";
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

  return (
    <>
      <div className="group/notebook-image my-4 w-full" ref={containerRef}>
        <div className="relative inline-block max-w-full">
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
            className={`absolute top-3 right-3 z-20 rounded-lg bg-black/50 text-white transition-opacity hover:bg-black/70 hover:text-white ${
              isTemp
                ? "opacity-100"
                : "opacity-0 group-hover/notebook-image:opacity-100 group-focus-within/notebook-image:opacity-100"
            }`}
          >
            <HugeiconsIcon
              icon={isTemp ? Cancel01Icon : Delete02Icon}
              strokeWidth={2}
            />
          </Button>

          {isTemp ? (
            <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-xl bg-black/20">
              <div className="size-7 animate-spin rounded-full border-2 border-white/40 border-t-white" />
            </div>
          ) : null}

          <div className="pointer-events-none absolute top-3 left-3 z-10 flex justify-start opacity-0 transition duration-150 group-hover/notebook-image:opacity-100 group-focus-within/notebook-image:opacity-100">
            <TooltipProvider>
              <div className="pointer-events-auto flex items-center gap-1.5 rounded-2xl border border-slate-200 bg-white/95 p-1.5 shadow-lg backdrop-blur">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="rounded-xl text-slate-600 hover:bg-slate-100 hover:text-slate-950"
                      aria-label="Download image"
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        handleDownload();
                      }}
                    >
                      <HugeiconsIcon icon={Download04Icon} strokeWidth={2.3} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom" sideOffset={8}>
                    Download
                  </TooltipContent>
                </Tooltip>

                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className={`rounded-xl ${
                        isResizeMode
                          ? "bg-slate-100 text-slate-950"
                          : "text-slate-600 hover:bg-slate-100 hover:text-slate-950"
                      }`}
                      aria-label="Resize image"
                      aria-pressed={isResizeMode}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        setIsResizeMode((current) => !current);
                      }}
                    >
                      <HugeiconsIcon icon={Resize01Icon} strokeWidth={2.3} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom" sideOffset={8}>
                    {isResizeMode ? "Exit resize" : "Resize"}
                  </TooltipContent>
                </Tooltip>

                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="rounded-xl text-slate-600 hover:bg-slate-100 hover:text-slate-950"
                      aria-label="Expand image"
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        setIsExpanded(true);
                      }}
                    >
                      <HugeiconsIcon
                        icon={ArrowExpandDiagonal01Icon}
                        strokeWidth={2.3}
                      />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom" sideOffset={8}>
                    Expand
                  </TooltipContent>
                </Tooltip>
              </div>
            </TooltipProvider>
          </div>

          <img
            ref={imageRef}
            src={src ?? ""}
            alt={altText}
            width={displayWidth || undefined}
            height={displayHeight || undefined}
            loading={isTemp ? "eager" : "lazy"}
            draggable={false}
            className="block h-auto max-h-[32rem] max-w-full object-contain"
            style={{
              width: displayWidth > 0 ? `${displayWidth}px` : "100%",
            }}
          />

          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  aria-label="Resize image"
                  onPointerDown={handleResizeStart}
                  className={`absolute right-3 bottom-3 z-10 flex size-9 cursor-se-resize items-center justify-center rounded-full border border-slate-200 bg-white/95 text-slate-600 shadow-lg transition duration-150 hover:bg-slate-100 hover:text-slate-950 ${
                    isResizeMode
                      ? "opacity-100"
                      : "opacity-0 group-hover/notebook-image:opacity-100 group-focus-within/notebook-image:opacity-100"
                  }`}
                  style={{ touchAction: "none" }}
                >
                  <HugeiconsIcon icon={Resize01Icon} strokeWidth={2.3} />
                </button>
              </TooltipTrigger>
              <TooltipContent side="left" sideOffset={8}>
                Drag to resize
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>

          {isResizing || isResizeMode ? (
            <div className="pointer-events-none absolute inset-0 rounded-xl ring-2 ring-slate-300" />
          ) : null}
        </div>
      </div>

      {isExpanded ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-6"
          onClick={(event) => {
            if (event.target === event.currentTarget) {
              setIsExpanded(false);
            }
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
          <img
            src={src ?? ""}
            alt={altText}
            className="max-h-[90vh] max-w-[90vw] object-contain"
          />
        </div>
      ) : null}
    </>
  );
}

export class NotebookImageNode extends DecoratorNode<JSX.Element> {
  __imageId: string;
  __src: string;
  __altText: string;
  __width: number;
  __height: number;

  static getType(): string {
    return "notebook-image";
  }

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

  static importJSON(
    serializedNode: SerializedNotebookImageNode,
  ): NotebookImageNode {
    return $createNotebookImageNode({
      imageId: serializedNode.imageId,
      src: serializedNode.src,
      altText: serializedNode.altText,
      width: serializedNode.width,
      height: serializedNode.height,
    });
  }

  constructor(
    imageId: string,
    src: string,
    altText: string,
    width: number,
    height: number,
    key?: NodeKey,
  ) {
    super(key);
    this.__imageId = imageId;
    this.__src = src;
    this.__altText = altText;
    this.__width = width;
    this.__height = height;
  }

  exportJSON(): SerializedNotebookImageNode {
    return {
      ...super.exportJSON(),
      type: "notebook-image",
      version: 1,
      imageId: this.__imageId,
      src: this.__src,
      altText: this.__altText,
      width: this.__width,
      height: this.__height,
    };
  }

  createDOM(): HTMLElement {
    return document.createElement("div");
  }

  updateDOM(): false {
    return false;
  }

  isInline(): false {
    return false;
  }

  decorate(): JSX.Element {
    return (
      <NotebookImageComponent
        nodeKey={this.getKey()}
        imageId={this.__imageId}
        src={this.__src}
        altText={this.__altText}
        width={this.__width}
        height={this.__height}
      />
    );
  }
}

export function $createNotebookImageNode({
  imageId,
  src,
  altText = "",
  width = 0,
  height = 0,
}: {
  imageId: string;
  src: string;
  altText?: string;
  width?: number;
  height?: number;
}): NotebookImageNode {
  return new NotebookImageNode(imageId, src, altText, width, height);
}

export function $isNotebookImageNode(
  node: LexicalNode | null | undefined,
): node is NotebookImageNode {
  return node instanceof NotebookImageNode;
}
