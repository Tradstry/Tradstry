import { useCallback, useEffect, useState } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isElementNode,
  $isRangeSelection,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  FORMAT_ELEMENT_COMMAND,
  FORMAT_TEXT_COMMAND,
  INDENT_CONTENT_COMMAND,
  OUTDENT_CONTENT_COMMAND,
  REDO_COMMAND,
  SELECTION_CHANGE_COMMAND,
  UNDO_COMMAND,
  type ElementFormatType,
  type TextFormatType,
} from "lexical";
import {
  $getSelectionStyleValueForProperty,
  $patchStyleText,
} from "@lexical/selection";
import { $isHeadingNode } from "@lexical/rich-text";
import { $isListNode, ListNode, REMOVE_LIST_COMMAND } from "@lexical/list";
import { $isLinkNode, TOGGLE_LINK_COMMAND } from "@lexical/link";
import { $getNearestNodeOfType, mergeRegister } from "@lexical/utils";
import {
  applyBlock,
  insertDivider,
  insertList,
  promptLink,
  type BlockKey,
  type ListKey,
} from "@tradstry/notebook-core/editor";
import {
  ArrowClockwiseIcon,
  ArrowCounterClockwiseIcon,
  CaretDownIcon,
  CodeIcon,
  DotsThreeIcon,
  HighlighterIcon,
  LinkIcon,
  ListBulletsIcon,
  ListChecksIcon,
  ListNumbersIcon,
  MinusIcon,
  ParagraphIcon,
  QuotesIcon,
  TextAlignCenterIcon,
  TextAlignJustifyIcon,
  TextAlignLeftIcon,
  TextAlignRightIcon,
  TextBIcon,
  TextHOneIcon,
  TextHThreeIcon,
  TextHTwoIcon,
  TextIndentIcon,
  TextItalicIcon,
  TextOutdentIcon,
  TextStrikethroughIcon,
  TextSubscriptIcon,
  TextSuperscriptIcon,
  TextTIcon,
  TextUnderlineIcon,
  type Icon,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

const BLOCKS: Record<BlockKey, { label: string; Icon: Icon }> = {
  paragraph: { label: "Text", Icon: ParagraphIcon },
  h1: { label: "Heading 1", Icon: TextHOneIcon },
  h2: { label: "Heading 2", Icon: TextHTwoIcon },
  h3: { label: "Heading 3", Icon: TextHThreeIcon },
  quote: { label: "Quote", Icon: QuotesIcon },
  code: { label: "Code block", Icon: CodeIcon },
};
const BLOCK_ORDER: BlockKey[] = ["paragraph", "h1", "h2", "h3", "quote", "code"];

const DEFAULT_FONT = "Geist Variable";
const FONTS = [
  { label: "Sans Serif", value: DEFAULT_FONT },
  { label: "Serif", value: "Georgia" },
  { label: "Monospace", value: "Menlo" },
];

const DEFAULT_SIZE = "15px";
const SIZES = [
  "12px",
  "13px",
  "14px",
  "15px",
  "16px",
  "18px",
  "20px",
  "24px",
  "30px",
];

const TEXT_COLORS: { label: string; value: string | null }[] = [
  { label: "Default", value: null },
  { label: "Red", value: "#ef4444" },
  { label: "Orange", value: "#f97316" },
  { label: "Amber", value: "#f59e0b" },
  { label: "Green", value: "#22c55e" },
  { label: "Blue", value: "#3b82f6" },
  { label: "Violet", value: "#8b5cf6" },
  { label: "Pink", value: "#ec4899" },
];

const HIGHLIGHTS: { label: string; value: string | null }[] = [
  { label: "None", value: null },
  { label: "Yellow", value: "#fef08a" },
  { label: "Green", value: "#bbf7d0" },
  { label: "Blue", value: "#bfdbfe" },
  { label: "Pink", value: "#fbcfe8" },
  { label: "Violet", value: "#ddd6fe" },
  { label: "Grey", value: "#e4e4e7" },
];

const ALIGNMENTS: { value: ElementFormatType; label: string; Icon: Icon }[] = [
  { value: "left", label: "Align left", Icon: TextAlignLeftIcon },
  { value: "center", label: "Align center", Icon: TextAlignCenterIcon },
  { value: "right", label: "Align right", Icon: TextAlignRightIcon },
  { value: "justify", label: "Justify", Icon: TextAlignJustifyIcon },
];

function Divider() {
  return (
    <span className="mx-1 h-4 w-px shrink-0 bg-zinc-200 dark:bg-zinc-800" />
  );
}

function ToolButton({
  label,
  active,
  ...props
}: React.ComponentProps<typeof Button> & { label: string; active?: boolean }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          aria-pressed={active}
          className={cn("shrink-0", active && "bg-accent text-foreground")}
          {...props}
        />
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}

function SwatchGrid({
  swatches,
  current,
  onPick,
}: {
  swatches: { label: string; value: string | null }[];
  current: string;
  onPick: (value: string | null) => void;
}) {
  return (
    <div className="grid grid-cols-4 gap-1">
      {swatches.map(({ label, value }) => {
        const selected = value ? current === value : !current;
        return (
          <Tooltip key={label}>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-label={label}
                aria-pressed={selected}
                onClick={() => onPick(value)}
                style={value ? { backgroundColor: value } : undefined}
                className={cn(
                  "flex size-7 cursor-pointer items-center justify-center rounded-md border border-zinc-200 outline-none transition focus-visible:ring-3 focus-visible:ring-ring/50 dark:border-zinc-700",
                  selected &&
                    "ring-2 ring-blue-500 ring-offset-1 ring-offset-background",
                )}
              >
                {value ? null : (
                  <MinusIcon size={13} className="text-muted-foreground" />
                )}
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom">{label}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}

export function Toolbar() {
  const [editor] = useLexicalComposerContext();
  const [block, setBlock] = useState<BlockKey>("paragraph");
  const [listType, setListType] = useState<ListKey | null>(null);
  const [formats, setFormats] = useState<Set<string>>(new Set());
  const [isLink, setIsLink] = useState(false);
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [align, setAlign] = useState<ElementFormatType>("left");
  const [font, setFont] = useState(DEFAULT_FONT);
  const [size, setSize] = useState(DEFAULT_SIZE);
  const [color, setColor] = useState("");
  const [highlight, setHighlight] = useState("");

  const update = useCallback(() => {
    const selection = $getSelection();
    if (!$isRangeSelection(selection)) return;

    const f = new Set<string>();
    (
      [
        "bold",
        "italic",
        "underline",
        "strikethrough",
        "code",
        "subscript",
        "superscript",
      ] as const
    ).forEach((fmt) => {
      if (selection.hasFormat(fmt)) f.add(fmt);
    });
    setFormats(f);

    const anchor = selection.anchor.getNode();
    const el =
      anchor.getKey() === "root" ? anchor : anchor.getTopLevelElementOrThrow();

    if ($isListNode(el)) {
      const parentList = $getNearestNodeOfType(anchor, ListNode);
      const type = (parentList ?? el).getListType();
      setListType(
        type === "number" ? "number" : type === "check" ? "check" : "bullet",
      );
      setBlock("paragraph");
    } else {
      setListType(null);
      if ($isHeadingNode(el)) {
        setBlock(el.getTag() as BlockKey);
      } else {
        const t = el.getType();
        setBlock(t === "quote" || t === "code" ? (t as BlockKey) : "paragraph");
      }
    }

    setAlign($isElementNode(el) ? el.getFormatType() || "left" : "left");
    setFont(
      $getSelectionStyleValueForProperty(selection, "font-family", DEFAULT_FONT),
    );
    setSize(
      $getSelectionStyleValueForProperty(selection, "font-size", DEFAULT_SIZE),
    );
    setColor($getSelectionStyleValueForProperty(selection, "color", ""));
    setHighlight(
      $getSelectionStyleValueForProperty(selection, "background-color", ""),
    );

    const parent = anchor.getParent();
    setIsLink($isLinkNode(anchor) || $isLinkNode(parent));
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

  const applyStyle = (styles: Record<string, string | null>) =>
    editor.update(() => {
      const s = $getSelection();
      if ($isRangeSelection(s)) $patchStyleText(s, styles);
    });

  // The toolbar toggles a list off when it's already active; the slash menu
  // only ever inserts, so that branch stays here rather than in insertList().
  const toggleList = (target: ListKey) => {
    if (listType === target) {
      editor.dispatchCommand(REMOVE_LIST_COMMAND, undefined);
      return;
    }
    insertList(editor, target);
  };

  const toggleFormat = (fmt: TextFormatType) =>
    editor.dispatchCommand(FORMAT_TEXT_COMMAND, fmt);

  const toggleLink = () => {
    if (isLink) {
      editor.dispatchCommand(TOGGLE_LINK_COMMAND, null);
      return;
    }
    promptLink(editor);
  };

  const Current = BLOCKS[block];
  const CurrentAlign = ALIGNMENTS.find((a) => a.value === align) ?? ALIGNMENTS[0];
  const fontLabel = FONTS.find((f) => f.value === font)?.label ?? FONTS[0].label;

  return (
    <div
      role="toolbar"
      aria-label="Formatting"
      aria-orientation="horizontal"
      className="flex items-center gap-1 overflow-x-auto border-b border-zinc-200/70 px-3 py-1.5 dark:border-zinc-800/70"
    >
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 shrink-0 gap-1.5 px-2 font-normal"
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
                onSelect={() => applyBlock(editor, key)}
                className={cn(block === key && "bg-accent")}
              >
                <Icon size={15} />
                {label}
              </DropdownMenuItem>
            );
          })}
        </DropdownMenuContent>
      </DropdownMenu>

      <Divider />

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label="Font"
            className="h-7 w-28 shrink-0 justify-between gap-1.5 px-2 font-normal"
          >
            <span className="truncate">{fontLabel}</span>
            <CaretDownIcon size={11} className="text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-40">
          {FONTS.map(({ label, value }) => (
            <DropdownMenuItem
              key={value}
              onSelect={() => applyStyle({ "font-family": value })}
              className={cn(font === value && "bg-accent")}
              style={{ fontFamily: value }}
            >
              {label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label="Font size"
            className="h-7 shrink-0 gap-1.5 px-2 font-normal tabular-nums"
          >
            {parseInt(size, 10)}
            <CaretDownIcon size={11} className="text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-20">
          {SIZES.map((value) => (
            <DropdownMenuItem
              key={value}
              onSelect={() => applyStyle({ "font-size": value })}
              className={cn("tabular-nums", size === value && "bg-accent")}
            >
              {parseInt(value, 10)}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      <Divider />

      <Popover>
        <PopoverTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label="Text color"
            className="relative shrink-0"
          >
            <TextTIcon size={15} />
            <span
              className="absolute inset-x-1.5 bottom-1 h-0.5 rounded-full"
              style={{ backgroundColor: color || "currentColor" }}
            />
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-auto p-2">
          <SwatchGrid
            swatches={TEXT_COLORS}
            current={color}
            onPick={(value) => applyStyle({ color: value })}
          />
        </PopoverContent>
      </Popover>

      <Popover>
        <PopoverTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label="Highlight"
            className="relative shrink-0"
          >
            <HighlighterIcon size={15} />
            <span
              className="absolute inset-x-1.5 bottom-1 h-0.5 rounded-full"
              style={{ backgroundColor: highlight || "currentColor" }}
            />
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-auto p-2">
          <SwatchGrid
            swatches={HIGHLIGHTS}
            current={highlight}
            onPick={(value) => applyStyle({ "background-color": value })}
          />
        </PopoverContent>
      </Popover>

      <Divider />

      <ToolButton
        label="Bold"
        active={formats.has("bold")}
        onClick={() => toggleFormat("bold")}
      >
        <TextBIcon size={15} />
      </ToolButton>
      <ToolButton
        label="Italic"
        active={formats.has("italic")}
        onClick={() => toggleFormat("italic")}
      >
        <TextItalicIcon size={15} />
      </ToolButton>
      <ToolButton
        label="Underline"
        active={formats.has("underline")}
        onClick={() => toggleFormat("underline")}
      >
        <TextUnderlineIcon size={15} />
      </ToolButton>

      <Divider />

      <ToolButton
        label="Bulleted list"
        active={listType === "bullet"}
        onClick={() => toggleList("bullet")}
      >
        <ListBulletsIcon size={15} />
      </ToolButton>
      <ToolButton
        label="Numbered list"
        active={listType === "number"}
        onClick={() => toggleList("number")}
      >
        <ListNumbersIcon size={15} />
      </ToolButton>
      <ToolButton
        label="Check list"
        active={listType === "check"}
        onClick={() => toggleList("check")}
      >
        <ListChecksIcon size={15} />
      </ToolButton>

      <Divider />

      <ToolButton label="Link" active={isLink} onClick={toggleLink}>
        <LinkIcon size={15} />
      </ToolButton>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label="Alignment"
            className="h-7 shrink-0 gap-1 px-1.5"
          >
            <CurrentAlign.Icon size={15} />
            <CaretDownIcon size={11} className="text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-40">
          {ALIGNMENTS.map(({ value, label, Icon }) => (
            <DropdownMenuItem
              key={value}
              onSelect={() =>
                editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, value)
              }
              className={cn(align === value && "bg-accent")}
            >
              <Icon size={15} />
              {label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      <ToolButton
        label="Outdent"
        onClick={() =>
          editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, undefined)
        }
      >
        <TextOutdentIcon size={15} />
      </ToolButton>
      <ToolButton
        label="Indent"
        onClick={() => editor.dispatchCommand(INDENT_CONTENT_COMMAND, undefined)}
      >
        <TextIndentIcon size={15} />
      </ToolButton>

      <Divider />

      <ToolButton
        label="Undo"
        disabled={!canUndo}
        onClick={() => editor.dispatchCommand(UNDO_COMMAND, undefined)}
      >
        <ArrowCounterClockwiseIcon size={15} />
      </ToolButton>
      <ToolButton
        label="Redo"
        disabled={!canRedo}
        onClick={() => editor.dispatchCommand(REDO_COMMAND, undefined)}
      >
        <ArrowClockwiseIcon size={15} />
      </ToolButton>

      <Divider />

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 shrink-0 gap-1 px-2 font-normal"
          >
            <DotsThreeIcon size={15} weight="bold" />
            More
            <CaretDownIcon size={11} className="text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-48">
          <DropdownMenuItem
            onSelect={() => toggleFormat("strikethrough")}
            className={cn(formats.has("strikethrough") && "bg-accent")}
          >
            <TextStrikethroughIcon size={15} />
            Strikethrough
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={() => toggleFormat("code")}
            className={cn(formats.has("code") && "bg-accent")}
          >
            <CodeIcon size={15} />
            Inline code
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={() => toggleFormat("superscript")}
            className={cn(formats.has("superscript") && "bg-accent")}
          >
            <TextSuperscriptIcon size={15} />
            Superscript
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={() => toggleFormat("subscript")}
            className={cn(formats.has("subscript") && "bg-accent")}
          >
            <TextSubscriptIcon size={15} />
            Subscript
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={() => insertDivider(editor)}>
            <MinusIcon size={15} />
            Horizontal rule
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
