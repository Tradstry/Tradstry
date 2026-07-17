"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $createParagraphNode, $isDecoratorNode, RootNode } from "lexical";
import { useEffect } from "react";

/**
 * Images and videos are block-level decorator nodes. When one is the last block,
 * there is no text node after it to place the caret in, so you can't start a new
 * line below the image. This keeps an empty paragraph trailing any block decorator
 * at the end of the document.
 */
export function TrailingParagraphPlugin() {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    return editor.registerNodeTransform(RootNode, (root) => {
      const last = root.getLastChild();
      if ($isDecoratorNode(last) && !last.isInline()) {
        root.append($createParagraphNode());
      }
    });
  }, [editor]);

  return null;
}
