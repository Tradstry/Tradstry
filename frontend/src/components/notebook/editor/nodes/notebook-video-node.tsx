"use client";

import { Delete02Icon } from "@hugeicons/core-free-icons";
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
import { type JSX, useState } from "react";
import { Button } from "@/components/ui/button";
import { useNotebookMediaActions } from "./notebook-image-node";

export type SerializedNotebookVideoNode = Spread<
  {
    type: "notebook-video";
    version: 1;
    videoId: string;
    src: string;
    altText: string;
  },
  SerializedLexicalNode
>;

function NotebookVideoComponent({
  nodeKey,
  videoId,
  src,
}: {
  nodeKey: NodeKey;
  videoId: string;
  src: string;
}) {
  const [editor] = useLexicalComposerContext();
  const { onDeleteImage } = useNotebookMediaActions();
  const [deleting, setDeleting] = useState(false);
  const isTemp = src?.startsWith("blob:") ?? false;

  const handleDelete = async () => {
    if (deleting) return;
    setDeleting(true);
    try {
      // Persisted videos (real id, not a temp local one) are removed from R2 too.
      if (!isTemp && !videoId.startsWith("local-") && onDeleteImage) {
        await onDeleteImage(videoId);
      }
      editor.update(() => {
        $getNodeByKey(nodeKey)?.remove();
      });
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="group relative my-2 inline-block max-w-full">
      {/** biome-ignore lint/a11y/useMediaCaption: user-pasted clips have no captions */}
      <video
        src={src}
        controls
        preload="metadata"
        className="max-h-[32rem] max-w-full rounded-lg"
      />
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label="Delete video"
        disabled={deleting}
        onClick={handleDelete}
        className="absolute top-2 right-2 bg-black/50 text-white opacity-0 transition-opacity hover:bg-black/70 hover:text-white group-hover:opacity-100"
      >
        <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
      </Button>
    </div>
  );
}

export class NotebookVideoNode extends DecoratorNode<JSX.Element> {
  __videoId: string;
  __src: string;
  __altText: string;

  static getType(): string {
    return "notebook-video";
  }

  static clone(node: NotebookVideoNode): NotebookVideoNode {
    return new NotebookVideoNode(
      node.__videoId,
      node.__src,
      node.__altText,
      node.__key,
    );
  }

  static importJSON(
    serializedNode: SerializedNotebookVideoNode,
  ): NotebookVideoNode {
    return $createNotebookVideoNode({
      videoId: serializedNode.videoId,
      src: serializedNode.src,
      altText: serializedNode.altText,
    });
  }

  constructor(videoId: string, src: string, altText: string, key?: NodeKey) {
    super(key);
    this.__videoId = videoId;
    this.__src = src;
    this.__altText = altText;
  }

  exportJSON(): SerializedNotebookVideoNode {
    return {
      ...super.exportJSON(),
      type: "notebook-video",
      version: 1,
      videoId: this.__videoId,
      src: this.__src,
      altText: this.__altText,
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
      <NotebookVideoComponent
        nodeKey={this.getKey()}
        videoId={this.__videoId}
        src={this.__src}
      />
    );
  }
}

export function $createNotebookVideoNode({
  videoId,
  src,
  altText = "",
}: {
  videoId: string;
  src: string;
  altText?: string;
}): NotebookVideoNode {
  return new NotebookVideoNode(videoId, src, altText);
}

export function $isNotebookVideoNode(
  node: LexicalNode | null | undefined,
): node is NotebookVideoNode {
  return node instanceof NotebookVideoNode;
}
