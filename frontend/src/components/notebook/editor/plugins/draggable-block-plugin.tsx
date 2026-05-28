"use client";

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { DraggableBlockPlugin_EXPERIMENTAL } from "@lexical/react/LexicalDraggableBlockPlugin";
import {
  $createParagraphNode,
  $getNearestNodeFromDOMNode,
  $getSelection,
  $isRangeSelection,
} from "lexical";
import { useEffect, useRef, useState } from "react";

const DRAGGABLE_BLOCK_MENU_CLASSNAME = "draggable-block-menu";

function isOnMenu(element: HTMLElement): boolean {
  return Boolean(element.closest(`.${DRAGGABLE_BLOCK_MENU_CLASSNAME}`));
}

// Six-dot drag affordance (matches the Notion-style block handle).
function DragDotsIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="currentColor"
      aria-hidden="true"
    >
      <circle cx="5.5" cy="4" r="1.3" />
      <circle cx="10.5" cy="4" r="1.3" />
      <circle cx="5.5" cy="8" r="1.3" />
      <circle cx="10.5" cy="8" r="1.3" />
      <circle cx="5.5" cy="12" r="1.3" />
      <circle cx="10.5" cy="12" r="1.3" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

/**
 * Notion-style hover gutter: a "+" (insert a block below) and a drag handle
 * (reorder blocks) that float to the left of whichever block the pointer is
 * over. Built on Lexical's experimental draggable-block plugin, which handles
 * positioning, the drop-target line, and the drag/drop reorder itself.
 */
export function DraggableBlockPlugin({
  anchorElem,
}: {
  anchorElem: HTMLElement;
}) {
  const [editor] = useLexicalComposerContext();
  const menuRef = useRef<HTMLDivElement>(null);
  const targetLineRef = useRef<HTMLDivElement>(null);
  const [draggableElement, setDraggableElement] = useState<HTMLElement | null>(
    null,
  );

  // Caret-follow: the underlying plugin targets the block under the mouse
  // (which is what enables drag-to-reorder). To ALSO make the handle track the
  // text caret, we synthesize a `mousemove` at the caret's block whenever the
  // selection moves to a new block — the plugin re-targets there, and real
  // mouse moves still override it so you can grab any block to drag.
  const lastBlockKeyRef = useRef<string | null>(null);
  useEffect(() => {
    const scroller = anchorElem.parentElement;
    if (!scroller) {
      return;
    }
    return editor.registerUpdateListener(() => {
      editor.getEditorState().read(() => {
        const selection = $getSelection();
        if (!$isRangeSelection(selection)) {
          return;
        }
        const topLevel = selection.anchor.getNode().getTopLevelElement();
        if (!topLevel) {
          return;
        }
        const key = topLevel.getKey();
        // Only reposition when the caret enters a different block, so typing
        // within a block doesn't re-fire on every keystroke.
        if (key === lastBlockKeyRef.current) {
          return;
        }
        lastBlockKeyRef.current = key;
        const dom = editor.getElementByKey(key);
        if (!dom) {
          return;
        }
        const rect = dom.getBoundingClientRect();
        scroller.dispatchEvent(
          new MouseEvent("mousemove", {
            bubbles: true,
            clientX: rect.left + 8,
            clientY: rect.top + Math.min(rect.height, 24) / 2,
          }),
        );
      });
    });
  }, [editor, anchorElem]);

  function insertBlockBelow() {
    if (!draggableElement) {
      return;
    }
    editor.update(() => {
      const node = $getNearestNodeFromDOMNode(draggableElement);
      if (!node) {
        return;
      }
      const topLevel = node.getTopLevelElementOrThrow();
      const paragraph = $createParagraphNode();
      topLevel.insertAfter(paragraph);
      paragraph.selectStart();
    });
    // Refocus the editor so the cursor lands in the new block.
    requestAnimationFrame(() => editor.focus());
  }

  return (
    <DraggableBlockPlugin_EXPERIMENTAL
      anchorElem={anchorElem}
      menuRef={menuRef}
      targetLineRef={targetLineRef}
      isOnMenu={isOnMenu}
      onElementChanged={setDraggableElement}
      menuComponent={
        <div
          ref={menuRef}
          className={`${DRAGGABLE_BLOCK_MENU_CLASSNAME} absolute top-0 left-0 flex items-center gap-0.5 rounded-md opacity-0 transition-opacity will-change-transform`}
        >
          <button
            type="button"
            title="Add a block below"
            onClick={insertBlockBelow}
            className="flex size-5 items-center justify-center rounded text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700"
          >
            <PlusIcon />
          </button>
          <span
            title="Drag to move"
            className="flex size-5 cursor-grab items-center justify-center rounded text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700 active:cursor-grabbing"
          >
            <DragDotsIcon />
          </span>
        </div>
      }
      targetLineComponent={
        <div
          ref={targetLineRef}
          className="draggable-block-target-line pointer-events-none absolute top-0 left-0 h-0.5 rounded-full bg-sky-500 opacity-0 will-change-transform"
        />
      }
    />
  );
}
