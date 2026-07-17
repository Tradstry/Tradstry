import type { EditorThemeClasses } from "lexical";

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
	// pre-wrap, not pre: Lexical sets `white-space: pre-wrap` inline on every text
	// span, and a wrapping child inside a non-wrapping parent mis-positions the caret.
	code: "my-3 block overflow-x-auto whitespace-pre-wrap rounded-lg bg-zinc-100 py-3 pl-3 pr-24 font-mono text-[13px] leading-6 [tab-size:2] text-zinc-800 dark:bg-zinc-800/70 dark:text-zinc-100",
	// Keyed by Prism token type; `registerCodeHighlighting` applies these.
	codeHighlight: {
		comment: "italic text-zinc-400 dark:text-zinc-500",
		prolog: "italic text-zinc-400 dark:text-zinc-500",
		doctype: "italic text-zinc-400 dark:text-zinc-500",
		cdata: "italic text-zinc-400 dark:text-zinc-500",
		punctuation: "text-zinc-500 dark:text-zinc-400",
		namespace: "opacity-70",
		property: "text-rose-600 dark:text-rose-400",
		tag: "text-rose-600 dark:text-rose-400",
		boolean: "text-rose-600 dark:text-rose-400",
		number: "text-rose-600 dark:text-rose-400",
		constant: "text-rose-600 dark:text-rose-400",
		symbol: "text-rose-600 dark:text-rose-400",
		deleted: "text-rose-600 dark:text-rose-400",
		selector: "text-emerald-600 dark:text-emerald-400",
		"attr-name": "text-emerald-600 dark:text-emerald-400",
		string: "text-emerald-600 dark:text-emerald-400",
		char: "text-emerald-600 dark:text-emerald-400",
		builtin: "text-emerald-600 dark:text-emerald-400",
		inserted: "text-emerald-600 dark:text-emerald-400",
		operator: "text-amber-600 dark:text-amber-400",
		entity: "text-amber-600 dark:text-amber-400",
		url: "text-amber-600 dark:text-amber-400",
		variable: "text-amber-600 dark:text-amber-400",
		atrule: "text-violet-600 dark:text-violet-400",
		"attr-value": "text-violet-600 dark:text-violet-400",
		keyword: "text-violet-600 dark:text-violet-400",
		function: "text-blue-600 dark:text-blue-400",
		"class-name": "text-blue-600 dark:text-blue-400",
		class: "text-blue-600 dark:text-blue-400",
		regex: "text-orange-600 dark:text-orange-400",
		important: "font-semibold text-orange-600 dark:text-orange-400",
	},
	link: "text-blue-600 underline underline-offset-2 hover:text-blue-500 dark:text-blue-400",
	hr: "my-6 h-px border-0 bg-zinc-200 dark:bg-zinc-800",
	tableScrollableWrapper: "my-3 overflow-x-auto",
	table: "w-full border-collapse text-[15px] text-zinc-700 dark:text-zinc-200",
	tableRow: "",
	tableCell:
		"min-w-[6rem] border border-zinc-200 px-3 py-2 align-top text-left dark:border-zinc-700 [&>p]:mb-0",
	tableCellHeader:
		"bg-zinc-100 font-semibold text-zinc-900 dark:bg-zinc-800/70 dark:text-zinc-50",
	tableSelection: "bg-blue-500/15",
	tableCellSelected: "bg-blue-500/10",
	text: {
		bold: "font-semibold",
		italic: "italic",
		underline: "underline underline-offset-2",
		strikethrough: "line-through",
		underlineStrikethrough: "underline line-through underline-offset-2",
		subscript: "align-sub text-[0.75em]",
		superscript: "align-super text-[0.75em]",
		code: "rounded bg-zinc-100 px-1 py-0.5 font-mono text-[0.85em] text-zinc-800 dark:bg-zinc-800 dark:text-zinc-100",
	},
};
