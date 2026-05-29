"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $getNodeByKey, $insertNodes } from "lexical";
import { useEffect } from "react";
import type { NotebookImage } from "@/lib/types/notebook";
import {
  $createNotebookImageNode,
  $isNotebookImageNode,
} from "../nodes/notebook-image-node";
import {
  $createNotebookVideoNode,
  $isNotebookVideoNode,
} from "../nodes/notebook-video-node";
import { registerUpload, unregisterUpload } from "../upload-registry";

function createTempImageId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `local-${crypto.randomUUID()}`;
  }
  return `local-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function PasteImagePlugin({
  onUploadImage,
}: {
  onUploadImage?: (file: File, signal?: AbortSignal) => Promise<NotebookImage>;
}) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    if (!onUploadImage) {
      return;
    }

    return editor.registerRootListener((rootElement, previousRootElement) => {
      if (previousRootElement) {
        previousRootElement.onpaste = null;
      }

      if (!rootElement) {
        return;
      }

      rootElement.onpaste = (event) => {
        const mediaFiles = new Map<string, File>();
        const isMedia = (type: string) =>
          type.startsWith("image/") || type.startsWith("video/");
        const clipboardFiles = Array.from(event.clipboardData?.files ?? []);
        const clipboardItems = Array.from(
          event.clipboardData?.items ?? [],
        ).filter((item) => item.kind === "file" && isMedia(item.type));

        for (const file of clipboardFiles) {
          if (isMedia(file.type)) {
            mediaFiles.set(`${file.name}:${file.size}:${file.type}`, file);
          }
        }

        for (const item of clipboardItems) {
          const file = item.getAsFile();
          if (!file || !isMedia(file.type)) continue;
          mediaFiles.set(`${file.name}:${file.size}:${file.type}`, file);
        }

        const files = Array.from(mediaFiles.values());
        if (files.length === 0) return;

        event.preventDefault();

        const pending = files.map((file) => ({
          file,
          localSrc: URL.createObjectURL(file),
          tempId: createTempImageId(),
          isVideo: file.type.startsWith("video/"),
          controller: new AbortController(),
        }));

        const nodeKeys: string[] = [];

        editor.update(() => {
          const nodes = pending.map(({ localSrc, tempId, file, isVideo }) => {
            const node = isVideo
              ? $createNotebookVideoNode({
                  videoId: tempId,
                  src: localSrc,
                  altText: file.name,
                })
              : $createNotebookImageNode({
                  imageId: tempId,
                  src: localSrc,
                  altText: file.name,
                });
            nodeKeys.push(node.getKey());
            return node;
          });
          $insertNodes(nodes);
        });

        void Promise.all(
          pending.map(
            async ({ file, localSrc, isVideo, tempId, controller }, i) => {
              // Make the upload cancellable: the node renders a cancel button that
              // calls abortUpload(tempId), which aborts this controller.
              registerUpload(tempId, controller);
              try {
                const uploaded: NotebookImage = await onUploadImage(
                  file,
                  controller.signal,
                );

                // Update node properties in place instead of replacing —
                // replacing changes the key and Lexical loses track of the node
                editor.update(() => {
                  const liveNode = $getNodeByKey(nodeKeys[i]!);
                  if (isVideo) {
                    if (!liveNode || !$isNotebookVideoNode(liveNode)) return;
                    const writable = liveNode.getWritable();
                    writable.__videoId = uploaded.id;
                    writable.__src = uploaded.secureUrl;
                    writable.__altText = uploaded.originalFilename || file.name;
                  } else {
                    if (!liveNode || !$isNotebookImageNode(liveNode)) return;
                    const writable = liveNode.getWritable();
                    writable.__imageId = uploaded.id;
                    writable.__src = uploaded.secureUrl;
                    writable.__altText = uploaded.originalFilename || file.name;
                    writable.__width = uploaded.width ?? 0;
                    writable.__height = uploaded.height ?? 0;
                  }
                });

                URL.revokeObjectURL(localSrc);
              } catch (error) {
                URL.revokeObjectURL(localSrc);

                editor.update(() => {
                  const liveNode = $getNodeByKey(nodeKeys[i]!);
                  if (!liveNode) return;
                  if (
                    $isNotebookVideoNode(liveNode) ||
                    $isNotebookImageNode(liveNode)
                  ) {
                    liveNode.remove();
                  }
                });

                console.error("Failed to upload pasted notebook media", error);
              } finally {
                unregisterUpload(tempId);
              }
            },
          ),
        );
      };
    });
  }, [editor, onUploadImage]);

  return null;
}
