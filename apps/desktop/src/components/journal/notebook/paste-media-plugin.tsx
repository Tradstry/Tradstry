import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  isImage,
  isVideo,
  makeThumbnail,
  MAX_IMAGE_BYTES,
  MAX_VIDEO_BYTES,
  probeDimensions,
  sha256Hex,
} from "@tradstry/notebook-core/media";
import { $insertNodes, COMMAND_PRIORITY_LOW, DROP_COMMAND } from "lexical";
import { useEffect } from "react";
import { storeMedia } from "../../../backend";
import { $createNotebookImageNode, $createNotebookVideoNode } from "./notebook-nodes";

const isMediaFile = (type: string) => isImage(type) || isVideo(type);

/**
 * Stores pasted/dropped media straight into the local media store (no HTTP
 * upload — desktop is local-first) and inserts a hash-addressed node. The
 * node's Yjs update syncs through the existing collaboration provider like any
 * other edit; `MediaResolverProvider` picks the hash up from there.
 */
export function PasteMediaPlugin({
  noteId,
  accountId,
}: {
  noteId: string;
  accountId: string;
}) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    async function storeAndInsert(file: File) {
      const video = isVideo(file.type);
      const cap = video ? MAX_VIDEO_BYTES : MAX_IMAGE_BYTES;
      if (file.size > cap) {
        console.warn(
          `Skipping ${video ? "video" : "image"} "${file.name}": ${file.size} bytes exceeds the ${cap} byte cap`,
        );
        return;
      }

      const buf = await file.arrayBuffer();
      const hash = await sha256Hex(buf);
      const dims = await probeDimensions(file);
      const thumbBlob = await makeThumbnail(file);

      try {
        await storeMedia(
          noteId,
          accountId,
          hash,
          file.type,
          video ? "video" : "image",
          dims.width,
          dims.height,
          dims.durationSeconds,
          file.name,
          new Uint8Array(buf),
          new Uint8Array(await thumbBlob.arrayBuffer()),
        );
      } catch (error) {
        console.error("Failed to store pasted notebook media", error);
        return;
      }

      editor.update(() => {
        const node = video
          ? $createNotebookVideoNode({ hash, altText: file.name })
          : $createNotebookImageNode({
              hash,
              altText: file.name,
              width: dims.width,
              height: dims.height,
            });
        $insertNodes([node]);
      });
    }

    const handleFiles = (files: File[]) => {
      if (files.length === 0) return;
      void Promise.all(files.map((file) => storeAndInsert(file)));
    };

    const unregisterRoot = editor.registerRootListener(
      (rootElement, previousRootElement) => {
        if (previousRootElement) {
          previousRootElement.onpaste = null;
        }
        if (!rootElement) return;

        rootElement.onpaste = (event) => {
          const files = Array.from(event.clipboardData?.files ?? []).filter((f) =>
            isMediaFile(f.type),
          );
          if (files.length === 0) return;
          event.preventDefault();
          handleFiles(files);
        };
      },
    );

    const unregisterDrop = editor.registerCommand(
      DROP_COMMAND,
      (event: DragEvent) => {
        const files = Array.from(event.dataTransfer?.files ?? []).filter((f) =>
          isMediaFile(f.type),
        );
        if (files.length === 0) return false;
        event.preventDefault();
        handleFiles(files);
        return true;
      },
      COMMAND_PRIORITY_LOW,
    );

    return () => {
      unregisterRoot();
      unregisterDrop();
    };
  }, [editor, noteId, accountId]);

  return null;
}
