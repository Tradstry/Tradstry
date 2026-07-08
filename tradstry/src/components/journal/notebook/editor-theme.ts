import { type EditorThemeClasses } from "lexical";

/**
 * A deliberately different skin from the web's Notion-style editor: a calm,
 * editorial writing surface — generous line-height, quiet headings, a colored
 * quote rail, and soft code blocks.
 */
export const editorTheme: EditorThemeClasses = {
  paragraph: "mb-2 text-[15px] leading-7 text-zinc-700 dark:text-zinc-200",
  heading: {
    h1: "mb-2 mt-6 text-2xl font-bold tracking-tight text-zinc-900 first:mt-0 dark:text-zinc-50",
    h2: "mb-2 mt-6 text-xl font-semibold tracking-tight text-zinc-900 first:mt-0 dark:text-zinc-50",
    h3: "mb-1.5 mt-5 text-base font-semibold text-zinc-900 first:mt-0 dark:text-zinc-50",
  },
  quote:
    "my-3 border-l-2 border-blue-500/70 pl-4 text-[15px] italic text-zinc-500 dark:text-zinc-400",
  list: {
    ul: "my-2 ml-5 list-disc space-y-1",
    ol: "my-2 ml-5 list-decimal space-y-1",
    listitem:
      "text-[15px] leading-7 text-zinc-700 marker:text-zinc-400 dark:text-zinc-200",
    listitemChecked: "editor-listitem-checked",
    listitemUnchecked: "editor-listitem-unchecked",
    nested: { listitem: "list-none" },
  },
  code: "my-3 block overflow-x-auto rounded-lg bg-zinc-100 p-3 font-mono text-[13px] leading-6 text-zinc-800 dark:bg-zinc-800/70 dark:text-zinc-100",
  link: "text-blue-600 underline underline-offset-2 hover:text-blue-500 dark:text-blue-400",
  text: {
    bold: "font-semibold",
    italic: "italic",
    underline: "underline underline-offset-2",
    strikethrough: "line-through",
    underlineStrikethrough: "underline line-through underline-offset-2",
    code: "rounded bg-zinc-100 px-1 py-0.5 font-mono text-[0.85em] text-zinc-800 dark:bg-zinc-800 dark:text-zinc-100",
  },
};
