import { useCallback, useEffect, useMemo, useState } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $getNodeByKey, $getRoot } from "lexical";
import { $isCodeNode } from "@lexical/code";
import {
  getCodeLanguageOptions,
  registerCodeHighlighting,
} from "@lexical/code-prism";
import { CaretDownIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

import { DEFAULT_CODE_LANGUAGE } from "@tradstry/notebook-core/editor";

export { DEFAULT_CODE_LANGUAGE };

/** Drives Prism tokenization; without it CodeHighlightNode is never populated. */
export function CodeHighlightPlugin() {
  const [editor] = useLexicalComposerContext();
  useEffect(() => registerCodeHighlighting(editor), [editor]);
  return null;
}

type Block = { key: string; language: string; top: number };

const same = (a: Block[], b: Block[]) =>
  a.length === b.length &&
  a.every(
    (x, i) =>
      x.key === b[i].key && x.language === b[i].language && x.top === b[i].top,
  );

/**
 * Renders a language dropdown pinned to the top-right of each code block.
 *
 * The chips are siblings of the ContentEditable, absolutely positioned against
 * the shared `relative` wrapper — never portalled *into* the code element,
 * whose children Lexical's reconciler owns.
 */
export function CodeLanguagePlugin() {
  const [editor] = useLexicalComposerContext();
  const [blocks, setBlocks] = useState<Block[]>([]);

  const languages = useMemo(
    () =>
      getCodeLanguageOptions().sort((a, b) => a[1].localeCompare(b[1])),
    [],
  );

  const measure = useCallback(() => {
    const next: Block[] = [];
    editor.getEditorState().read(() => {
      for (const node of $getRoot().getChildren()) {
        if (!$isCodeNode(node)) continue;
        const key = node.getKey();
        const el = editor.getElementByKey(key);
        if (!el) continue;
        next.push({
          key,
          language: node.getLanguage() || DEFAULT_CODE_LANGUAGE,
          top: el.offsetTop,
        });
      }
    });
    setBlocks((prev) => (same(prev, next) ? prev : next));
  }, [editor]);

  useEffect(() => {
    measure();
    const unregister = editor.registerUpdateListener(measure);
    const root = editor.getRootElement();
    const observer = new ResizeObserver(measure);
    if (root) observer.observe(root);
    window.addEventListener("resize", measure);
    return () => {
      unregister();
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [editor, measure]);

  const setLanguage = (key: string, language: string) =>
    editor.update(() => {
      const node = $getNodeByKey(key);
      if ($isCodeNode(node)) node.setLanguage(language);
    });

  const labelFor = (value: string) =>
    languages.find(([v]) => v === value)?.[1] ?? value;

  return (
    <>
      {blocks.map(({ key, language, top }) => (
        <div key={key} style={{ top: top + 6 }} className="absolute right-1.5 z-10">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="xs"
                aria-label={`Language: ${labelFor(language)}`}
                className="gap-1 border border-zinc-200/80 bg-white/70 px-1.5 font-normal text-muted-foreground backdrop-blur-sm hover:text-foreground dark:border-zinc-700/80 dark:bg-zinc-900/70"
              >
                {labelFor(language)}
                <CaretDownIcon size={10} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="max-h-64 w-40">
              {languages.map(([value, label]) => (
                <DropdownMenuItem
                  key={value}
                  onSelect={() => setLanguage(key, value)}
                  className={cn(language === value && "bg-accent")}
                >
                  {label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      ))}
    </>
  );
}
