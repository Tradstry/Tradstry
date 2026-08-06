"use client";

import { $createCodeNode, $isCodeNode } from "@lexical/code";
import { $toggleLink } from "@lexical/link";
import {
  $isListNode,
  INSERT_ORDERED_LIST_COMMAND,
  INSERT_UNORDERED_LIST_COMMAND,
  ListNode,
} from "@lexical/list";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $createHeadingNode,
  $createQuoteNode,
  $isHeadingNode,
  $isQuoteNode,
} from "@lexical/rich-text";
import { $setBlocksType } from "@lexical/selection";
import { $getNearestNodeOfType, mergeRegister } from "@lexical/utils";
import {
  $createParagraphNode,
  $getSelection,
  $isRangeSelection,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  COMMAND_PRIORITY_LOW,
  FORMAT_TEXT_COMMAND,
  REDO_COMMAND,
  SELECTION_CHANGE_COMMAND,
  UNDO_COMMAND,
} from "lexical";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { cn } from "@tradstry/app-ui/lib/utils";

type BlockType =
  | "paragraph"
  | "h1"
  | "h2"
  | "quote"
  | "bullet"
  | "number"
  | "code";

function ToolbarButton({
  label,
  isActive = false,
  onClick,
  disabled = false,
}: {
  label: ReactNode;
  isActive?: boolean;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-pressed={isActive}
      className={cn(
        "h-8 min-w-8 rounded-md px-2 text-[13px] font-medium text-muted-foreground transition-colors",
        "hover:bg-accent hover:text-foreground disabled:opacity-40",
        isActive && "bg-accent text-foreground",
      )}
      onClick={onClick}
      disabled={disabled}
    >
      {label}
    </Button>
  );
}

export function ToolbarPlugin() {
  const [editor] = useLexicalComposerContext();
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [isBold, setIsBold] = useState(false);
  const [isItalic, setIsItalic] = useState(false);
  const [isUnderline, setIsUnderline] = useState(false);
  const [isCode, setIsCode] = useState(false);
  const [blockType, setBlockType] = useState<BlockType>("paragraph");

  const updateToolbar = useCallback(() => {
    const selection = $getSelection();
    if (!$isRangeSelection(selection)) {
      return;
    }

    setIsBold(selection.hasFormat("bold"));
    setIsItalic(selection.hasFormat("italic"));
    setIsUnderline(selection.hasFormat("underline"));
    setIsCode(selection.hasFormat("code"));

    const anchorNode = selection.anchor.getNode();
    const element =
      anchorNode.getKey() === "root"
        ? anchorNode
        : anchorNode.getTopLevelElementOrThrow();

    if ($isListNode(element)) {
      setBlockType(element.getListType() === "number" ? "number" : "bullet");
      return;
    }

    const nearestList = $getNearestNodeOfType(anchorNode, ListNode);
    if (nearestList) {
      setBlockType(
        nearestList.getListType() === "number" ? "number" : "bullet",
      );
      return;
    }

    if ($isHeadingNode(element)) {
      const tag = element.getTag();
      setBlockType(tag === "h1" ? "h1" : tag === "h2" ? "h2" : "paragraph");
      return;
    }

    if ($isQuoteNode(element)) {
      setBlockType("quote");
      return;
    }

    if ($isCodeNode(element)) {
      setBlockType("code");
      return;
    }

    setBlockType("paragraph");
  }, []);

  useEffect(() => {
    return mergeRegister(
      editor.registerUpdateListener(({ editorState }) => {
        editorState.read(() => {
          updateToolbar();
        });
      }),
      editor.registerCommand(
        SELECTION_CHANGE_COMMAND,
        () => {
          updateToolbar();
          return false;
        },
        COMMAND_PRIORITY_LOW,
      ),
      editor.registerCommand(
        CAN_UNDO_COMMAND,
        (payload) => {
          setCanUndo(payload);
          return false;
        },
        COMMAND_PRIORITY_LOW,
      ),
      editor.registerCommand(
        CAN_REDO_COMMAND,
        (payload) => {
          setCanRedo(payload);
          return false;
        },
        COMMAND_PRIORITY_LOW,
      ),
    );
  }, [editor, updateToolbar]);

  const applyBlockType = useCallback(
    (nextBlockType: BlockType) => {
      editor.update(() => {
        const selection = $getSelection();
        if (!$isRangeSelection(selection)) {
          return;
        }

        switch (nextBlockType) {
          case "paragraph":
            $setBlocksType(selection, () => $createParagraphNode());
            break;
          case "h1":
            $setBlocksType(selection, () => $createHeadingNode("h1"));
            break;
          case "h2":
            $setBlocksType(selection, () => $createHeadingNode("h2"));
            break;
          case "quote":
            $setBlocksType(selection, () => $createQuoteNode());
            break;
          case "code":
            $setBlocksType(selection, () => $createCodeNode());
            break;
          case "bullet":
            editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined);
            break;
          case "number":
            editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND, undefined);
            break;
        }
      });
    },
    [editor],
  );

  const toggleLink = useCallback(() => {
    const url = window.prompt("Enter a URL");
    if (url === null) {
      return;
    }

    editor.update(() => {
      $toggleLink(url.trim() ? url.trim() : null);
    });
  }, [editor]);

  return (
    <div className="sticky top-0 z-10 flex shrink-0 flex-wrap items-center gap-0.5 border-b border-border/60 bg-background/80 px-3 py-1.5 backdrop-blur">
      <ToolbarButton
        label="Undo"
        onClick={() => editor.dispatchCommand(UNDO_COMMAND, undefined)}
        disabled={!canUndo}
      />
      <ToolbarButton
        label="Redo"
        onClick={() => editor.dispatchCommand(REDO_COMMAND, undefined)}
        disabled={!canRedo}
      />

      <div className="mx-1.5 h-5 w-px bg-border" />

      <ToolbarButton
        label={<span className="font-bold">B</span>}
        isActive={isBold}
        onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, "bold")}
      />
      <ToolbarButton
        label={<span className="italic">I</span>}
        isActive={isItalic}
        onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, "italic")}
      />
      <ToolbarButton
        label={<span className="underline">U</span>}
        isActive={isUnderline}
        onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, "underline")}
      />
      <ToolbarButton
        label="Code"
        isActive={isCode}
        onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, "code")}
      />

      <div className="mx-1.5 h-5 w-px bg-border" />

      <ToolbarButton
        label="Text"
        isActive={blockType === "paragraph"}
        onClick={() => applyBlockType("paragraph")}
      />
      <ToolbarButton
        label="H1"
        isActive={blockType === "h1"}
        onClick={() => applyBlockType("h1")}
      />
      <ToolbarButton
        label="H2"
        isActive={blockType === "h2"}
        onClick={() => applyBlockType("h2")}
      />
      <ToolbarButton
        label="Quote"
        isActive={blockType === "quote"}
        onClick={() => applyBlockType("quote")}
      />
      <ToolbarButton
        label="Bullets"
        isActive={blockType === "bullet"}
        onClick={() => applyBlockType("bullet")}
      />
      <ToolbarButton
        label="Numbers"
        isActive={blockType === "number"}
        onClick={() => applyBlockType("number")}
      />
      <ToolbarButton
        label="Block Code"
        isActive={blockType === "code"}
        onClick={() => applyBlockType("code")}
      />
      <ToolbarButton label="Link" onClick={toggleLink} />
    </div>
  );
}
