import { useCallback, useEffect, useState } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $createParagraphNode,
  $getSelection,
  $isRangeSelection,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  FORMAT_TEXT_COMMAND,
  REDO_COMMAND,
  SELECTION_CHANGE_COMMAND,
  UNDO_COMMAND,
  type TextFormatType,
} from "lexical";
import { $setBlocksType } from "@lexical/selection";
import {
  $createHeadingNode,
  $createQuoteNode,
  $isHeadingNode,
  type HeadingTagType,
} from "@lexical/rich-text";
import { $createCodeNode } from "@lexical/code";
import {
  $isListNode,
  INSERT_CHECK_LIST_COMMAND,
  INSERT_ORDERED_LIST_COMMAND,
  INSERT_UNORDERED_LIST_COMMAND,
  ListNode,
  REMOVE_LIST_COMMAND,
} from "@lexical/list";
import { $isLinkNode, TOGGLE_LINK_COMMAND } from "@lexical/link";
import { $getNearestNodeOfType, mergeRegister } from "@lexical/utils";
import {
  ArrowClockwiseIcon,
  ArrowCounterClockwiseIcon,
  CaretDownIcon,
  CodeIcon,
  LinkIcon,
  ListBulletsIcon,
  ListChecksIcon,
  ListNumbersIcon,
  ParagraphIcon,
  QuotesIcon,
  TextBIcon,
  TextHOneIcon,
  TextHThreeIcon,
  TextHTwoIcon,
  TextItalicIcon,
  TextStrikethroughIcon,
  TextUnderlineIcon,
  type Icon,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

type BlockKey =
  | "paragraph"
  | "h1"
  | "h2"
  | "h3"
  | "bullet"
  | "number"
  | "check"
  | "quote"
  | "code";

const BLOCKS: Record<BlockKey, { label: string; Icon: Icon }> = {
  paragraph: { label: "Text", Icon: ParagraphIcon },
  h1: { label: "Heading 1", Icon: TextHOneIcon },
  h2: { label: "Heading 2", Icon: TextHTwoIcon },
  h3: { label: "Heading 3", Icon: TextHThreeIcon },
  bullet: { label: "Bulleted list", Icon: ListBulletsIcon },
  number: { label: "Numbered list", Icon: ListNumbersIcon },
  check: { label: "Check list", Icon: ListChecksIcon },
  quote: { label: "Quote", Icon: QuotesIcon },
  code: { label: "Code", Icon: CodeIcon },
};
const BLOCK_ORDER: BlockKey[] = [
  "paragraph",
  "h1",
  "h2",
  "h3",
  "bullet",
  "number",
  "check",
  "quote",
  "code",
];

export function Toolbar() {
  const [editor] = useLexicalComposerContext();
  const [block, setBlock] = useState<BlockKey>("paragraph");
  const [formats, setFormats] = useState<Set<string>>(new Set());
  const [isLink, setIsLink] = useState(false);
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);

  const update = useCallback(() => {
    const selection = $getSelection();
    if (!$isRangeSelection(selection)) return;

    const f = new Set<string>();
    (["bold", "italic", "underline", "strikethrough", "code"] as const).forEach(
      (fmt) => {
        if (selection.hasFormat(fmt)) f.add(fmt);
      },
    );
    setFormats(f);

    const anchor = selection.anchor.getNode();
    const el =
      anchor.getKey() === "root" ? anchor : anchor.getTopLevelElementOrThrow();

    if ($isListNode(el)) {
      const parentList = $getNearestNodeOfType(anchor, ListNode);
      const type = (parentList ?? el).getListType();
      setBlock(type === "number" ? "number" : type === "check" ? "check" : "bullet");
    } else if ($isHeadingNode(el)) {
      setBlock(el.getTag() as BlockKey);
    } else {
      const t = el.getType();
      setBlock(t === "quote" || t === "code" ? (t as BlockKey) : "paragraph");
    }

    const node = selection.anchor.getNode();
    const parent = node.getParent();
    setIsLink($isLinkNode(node) || $isLinkNode(parent));
  }, []);

  useEffect(() => {
    return mergeRegister(
      editor.registerUpdateListener(({ editorState }) => {
        editorState.read(update);
      }),
      editor.registerCommand(
        SELECTION_CHANGE_COMMAND,
        () => {
          update();
          return false;
        },
        1,
      ),
      editor.registerCommand(
        CAN_UNDO_COMMAND,
        (payload) => {
          setCanUndo(payload);
          return false;
        },
        1,
      ),
      editor.registerCommand(
        CAN_REDO_COMMAND,
        (payload) => {
          setCanRedo(payload);
          return false;
        },
        1,
      ),
    );
  }, [editor, update]);

  const setParagraph = () =>
    editor.update(() => {
      const s = $getSelection();
      if ($isRangeSelection(s)) $setBlocksType(s, () => $createParagraphNode());
    });
  const setHeading = (tag: HeadingTagType) =>
    editor.update(() => {
      const s = $getSelection();
      if ($isRangeSelection(s)) $setBlocksType(s, () => $createHeadingNode(tag));
    });
  const setQuote = () =>
    editor.update(() => {
      const s = $getSelection();
      if ($isRangeSelection(s)) $setBlocksType(s, () => $createQuoteNode());
    });
  const setCode = () =>
    editor.update(() => {
      const s = $getSelection();
      if ($isRangeSelection(s)) $setBlocksType(s, () => $createCodeNode());
    });

  const toggleList = (target: "bullet" | "number" | "check") => {
    if (block === target) {
      editor.dispatchCommand(REMOVE_LIST_COMMAND, undefined);
      return;
    }
    const command =
      target === "bullet"
        ? INSERT_UNORDERED_LIST_COMMAND
        : target === "number"
          ? INSERT_ORDERED_LIST_COMMAND
          : INSERT_CHECK_LIST_COMMAND;
    editor.dispatchCommand(command, undefined);
  };

  const applyBlock = (key: BlockKey) => {
    switch (key) {
      case "paragraph":
        return setParagraph();
      case "h1":
      case "h2":
      case "h3":
        return setHeading(key);
      case "quote":
        return setQuote();
      case "code":
        return setCode();
      case "bullet":
      case "number":
      case "check":
        return toggleList(key);
    }
  };

  const toggleFormat = (fmt: TextFormatType) =>
    editor.dispatchCommand(FORMAT_TEXT_COMMAND, fmt);

  const toggleLink = () => {
    if (isLink) {
      editor.dispatchCommand(TOGGLE_LINK_COMMAND, null);
      return;
    }
    const url = window.prompt("Link URL");
    if (url) editor.dispatchCommand(TOGGLE_LINK_COMMAND, url);
  };

  const Current = BLOCKS[block];

  const inlineButtons: { fmt: TextFormatType; Icon: Icon; label: string }[] = [
    { fmt: "bold", Icon: TextBIcon, label: "Bold" },
    { fmt: "italic", Icon: TextItalicIcon, label: "Italic" },
    { fmt: "underline", Icon: TextUnderlineIcon, label: "Underline" },
    { fmt: "strikethrough", Icon: TextStrikethroughIcon, label: "Strikethrough" },
    { fmt: "code", Icon: CodeIcon, label: "Inline code" },
  ];

  return (
    <div className="flex items-center gap-1 border-b border-zinc-200/70 px-3 py-1.5 dark:border-zinc-800/70">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 gap-1.5 px-2 font-normal"
          >
            <Current.Icon size={15} />
            {Current.label}
            <CaretDownIcon size={11} className="text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-44">
          {BLOCK_ORDER.map((key) => {
            const { label, Icon } = BLOCKS[key];
            return (
              <DropdownMenuItem
                key={key}
                onSelect={() => applyBlock(key)}
                className={cn(block === key && "bg-accent")}
              >
                <Icon size={15} />
                {label}
              </DropdownMenuItem>
            );
          })}
        </DropdownMenuContent>
      </DropdownMenu>

      <span className="mx-1 h-4 w-px bg-zinc-200 dark:bg-zinc-800" />

      {inlineButtons.map(({ fmt, Icon, label }) => (
        <Button
          key={fmt}
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          aria-pressed={formats.has(fmt)}
          className={cn(formats.has(fmt) && "bg-accent text-foreground")}
          onClick={() => toggleFormat(fmt)}
        >
          <Icon size={15} />
        </Button>
      ))}
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label="Link"
        aria-pressed={isLink}
        className={cn(isLink && "bg-accent text-foreground")}
        onClick={toggleLink}
      >
        <LinkIcon size={15} />
      </Button>

      <span className="mx-1 h-4 w-px bg-zinc-200 dark:bg-zinc-800" />

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label="Undo"
        disabled={!canUndo}
        onClick={() => editor.dispatchCommand(UNDO_COMMAND, undefined)}
      >
        <ArrowCounterClockwiseIcon size={15} />
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label="Redo"
        disabled={!canRedo}
        onClick={() => editor.dispatchCommand(REDO_COMMAND, undefined)}
      >
        <ArrowClockwiseIcon size={15} />
      </Button>
    </div>
  );
}
