import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  LexicalTypeaheadMenuPlugin,
  MenuOption,
  useBasicTypeaheadTriggerMatch,
} from "@lexical/react/LexicalTypeaheadMenuPlugin";
import { type LexicalEditor } from "lexical";
import {
  CodeIcon,
  LinkIcon,
  ListBulletsIcon,
  ListChecksIcon,
  ListNumbersIcon,
  MinusIcon,
  ParagraphIcon,
  QuotesIcon,
  TextHOneIcon,
  TextHThreeIcon,
  TextHTwoIcon,
  type Icon,
} from "@phosphor-icons/react";
import {
  applyBlock,
  insertDivider,
  insertList,
  promptLink,
} from "@tradstry/notebook-core/editor";
import { ScrollArea } from "../../user-interface";
import { cn } from "@/lib/utils";

type Group = "Basic" | "Lists" | "Insert";

class SlashOption extends MenuOption {
  label: string;
  group: Group;
  Glyph: Icon;
  keywords: string[];
  run: (editor: LexicalEditor) => void;

  constructor(
    key: string,
    label: string,
    group: Group,
    Glyph: Icon,
    keywords: string[],
    run: (editor: LexicalEditor) => void,
  ) {
    super(key);
    this.label = label;
    this.group = group;
    this.Glyph = Glyph;
    this.keywords = keywords;
    this.run = run;
  }
}

const OPTIONS: SlashOption[] = [
  new SlashOption("paragraph", "Text", "Basic", ParagraphIcon, ["plain", "body"], (e) =>
    applyBlock(e, "paragraph"),
  ),
  new SlashOption("h1", "Heading 1", "Basic", TextHOneIcon, ["title", "big"], (e) =>
    applyBlock(e, "h1"),
  ),
  new SlashOption("h2", "Heading 2", "Basic", TextHTwoIcon, ["subtitle"], (e) =>
    applyBlock(e, "h2"),
  ),
  new SlashOption("h3", "Heading 3", "Basic", TextHThreeIcon, ["subheading"], (e) =>
    applyBlock(e, "h3"),
  ),
  new SlashOption("quote", "Quote", "Basic", QuotesIcon, ["blockquote", "cite"], (e) =>
    applyBlock(e, "quote"),
  ),
  new SlashOption("code", "Code block", "Basic", CodeIcon, ["snippet", "pre"], (e) =>
    applyBlock(e, "code"),
  ),
  new SlashOption("bullet", "Bulleted list", "Lists", ListBulletsIcon, ["ul", "unordered"], (e) =>
    insertList(e, "bullet"),
  ),
  new SlashOption("number", "Numbered list", "Lists", ListNumbersIcon, ["ol", "ordered"], (e) =>
    insertList(e, "number"),
  ),
  new SlashOption("check", "Check list", "Lists", ListChecksIcon, ["todo", "task"], (e) =>
    insertList(e, "check"),
  ),
  new SlashOption("divider", "Divider", "Insert", MinusIcon, ["hr", "rule", "separator"], (e) =>
    insertDivider(e),
  ),
  new SlashOption("link", "Link", "Insert", LinkIcon, ["url", "href", "anchor"], (e) =>
    promptLink(e),
  ),
];

function matches(option: SlashOption, query: string) {
  const q = query.toLowerCase();
  return (
    option.label.toLowerCase().includes(q) ||
    option.keywords.some((k) => k.includes(q))
  );
}

function SlashMenu({
  options,
  query,
  selectedIndex,
  onPick,
  onHighlight,
}: {
  options: SlashOption[];
  query: string | null;
  selectedIndex: number | null;
  onPick: (option: SlashOption) => void;
  onHighlight: (index: number) => void;
}) {
  const activeRef = useRef<HTMLDivElement | null>(null);

  // A custom menuRenderFn owns its own scrolling — Lexical only auto-scrolls
  // for its default renderer.
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  if (options.length === 0) {
    return (
      <div className="z-50 w-64 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg">
        <p className="px-2 py-6 text-center text-xs text-muted-foreground">
          No matching commands
        </p>
      </div>
    );
  }

  return (
    <div className="z-50 w-64 overflow-hidden rounded-lg border border-border bg-popover text-popover-foreground shadow-lg">
      {/* The viewport must be bounded, not the Root: Radix gives it
          `overflow-y: scroll` but `h-full` against an auto-height Root is auto. */}
      <ScrollArea className="max-h-72 [&>[data-slot=scroll-area-viewport]]:max-h-72">
        <div role="listbox" aria-label="Slash commands" className="p-1">
          {options.map((option, i) => {
            // Headings only when unfiltered — a short filtered list reads badly
            // under three separate section labels.
            const heading =
              !query && (i === 0 || options[i - 1].group !== option.group);
            const selected = selectedIndex === i;
            return (
              <div key={option.key}>
                {heading && (
                  <div className="px-2 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    {option.group}
                  </div>
                )}
                <div
                  role="option"
                  aria-selected={selected}
                  tabIndex={-1}
                  ref={(el) => {
                    option.setRefElement(el);
                    if (selected) activeRef.current = el;
                  }}
                  onMouseEnter={() => onHighlight(i)}
                  onClick={() => onPick(option)}
                  className={cn(
                    "flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-1.5 text-sm outline-none",
                    selected
                      ? "bg-accent text-accent-foreground"
                      : "text-zinc-700 dark:text-zinc-200",
                  )}
                >
                  <span className="flex size-6 shrink-0 items-center justify-center rounded border border-border bg-muted/50 text-muted-foreground">
                    <option.Glyph size={14} />
                  </span>
                  <span className="truncate">{option.label}</span>
                </div>
              </div>
            );
          })}
        </div>
      </ScrollArea>
    </div>
  );
}

export function SlashMenuPlugin() {
  const [editor] = useLexicalComposerContext();
  const [query, setQuery] = useState<string | null>(null);

  const triggerFn = useBasicTypeaheadTriggerMatch("/", { minLength: 0 });

  const options = useMemo(
    () => (query ? OPTIONS.filter((o) => matches(o, query)) : OPTIONS),
    [query],
  );

  const onSelectOption = useCallback(
    (
      option: SlashOption,
      nodeToRemove: import("lexical").TextNode | null,
      closeMenu: () => void,
    ) => {
      editor.update(() => nodeToRemove?.remove());
      closeMenu();
      // Runs after the menu unmounts so the Link prompt doesn't fight for focus.
      option.run(editor);
    },
    [editor],
  );

  return (
    <LexicalTypeaheadMenuPlugin<SlashOption>
      options={options}
      onQueryChange={setQuery}
      onSelectOption={onSelectOption}
      triggerFn={triggerFn}
      menuRenderFn={(
        anchorElementRef,
        { selectedIndex, selectOptionAndCleanUp, setHighlightedIndex },
      ) =>
        anchorElementRef.current
          ? createPortal(
              <SlashMenu
                options={options}
                query={query}
                selectedIndex={selectedIndex}
                onPick={selectOptionAndCleanUp}
                onHighlight={setHighlightedIndex}
              />,
              anchorElementRef.current,
            )
          : null
      }
    />
  );
}
