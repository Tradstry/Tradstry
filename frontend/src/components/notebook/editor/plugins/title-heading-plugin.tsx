"use client";

import { $createHeadingNode } from "@lexical/rich-text";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $isParagraphNode, RootNode } from "lexical";
import { useEffect } from "react";

/**
 * The first block of a note is its title: always an `h1`. This transform keeps it
 * that way — a fresh note seeds with an `h1`, but merging the title into the line
 * below (backspace) or an older note without one would otherwise leave a plain
 * paragraph on top. Converting in a node transform means the caret lands in an
 * `h1` the moment you enter the first line.
 */
export function TitleHeadingPlugin() {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    return editor.registerNodeTransform(RootNode, (root) => {
      // Only a plain paragraph is promoted. A quote/list/code the user put on the
      // first line stays as-is; a heading already is one.
      const first = root.getFirstChild();
      if (!$isParagraphNode(first)) {
        return;
      }

      const heading = $createHeadingNode("h1");
      heading.append(...first.getChildren());
      first.replace(heading);
    });
  }, [editor]);

  return null;
}
