import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { CheckListPlugin } from "@lexical/react/LexicalCheckListPlugin";
import { LinkPlugin } from "@lexical/react/LexicalLinkPlugin";
import { TabIndentationPlugin } from "@lexical/react/LexicalTabIndentationPlugin";
import { MarkdownShortcutPlugin } from "@lexical/react/LexicalMarkdownShortcutPlugin";
import { OnChangePlugin } from "@lexical/react/LexicalOnChangePlugin";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { ListItemNode, ListNode } from "@lexical/list";
import { CodeHighlightNode, CodeNode } from "@lexical/code";
import { AutoLinkNode, LinkNode } from "@lexical/link";
import { TRANSFORMERS } from "@lexical/markdown";
import { type ReactNode } from "react";
import { editorTheme } from "./editor-theme";
import { Toolbar } from "./toolbar";

/**
 * A Lexical block editor with a custom skin. Block editing works via Markdown
 * shortcuts ("# " heading, "- " list, "> " quote, "```" code, "[] " checklist)
 * plus the toolbar. `documentJson` is a Lexical SerializedEditorState string.
 *
 * NOTE: mount this with `key={note.id}` so switching notes reloads content —
 * LexicalComposer only reads `initialConfig` once.
 */
export function NotebookEditor({
  documentJson,
  onChange,
  header,
}: {
  documentJson: string;
  onChange: (json: string) => void;
  /** Rendered above the body inside the writing column (e.g. the title). */
  header?: ReactNode;
}) {
  return (
    <LexicalComposer
      initialConfig={{
        namespace: "tradstry-notebook",
        theme: editorTheme,
        editorState: documentJson || undefined,
        onError: (error) => console.error(error),
        nodes: [
          HeadingNode,
          QuoteNode,
          ListNode,
          ListItemNode,
          CodeNode,
          CodeHighlightNode,
          AutoLinkNode,
          LinkNode,
        ],
      }}
    >
      <div className="flex h-full min-h-0 flex-col">
        <Toolbar />
        <div className="min-h-0 flex-1 overflow-auto">
          <div className="mx-auto max-w-2xl px-8 py-6">
            {header}
            <div className="relative">
              <RichTextPlugin
                contentEditable={
                  <ContentEditable
                    spellCheck
                    className="min-h-[55vh] text-[15px] leading-7 text-zinc-700 outline-none dark:text-zinc-200"
                  />
                }
                placeholder={
                  <div className="pointer-events-none absolute left-0 top-0 text-[15px] text-zinc-400 dark:text-zinc-600">
                    Start writing… try “# ” for a heading or “- ” for a list.
                  </div>
                }
                ErrorBoundary={LexicalErrorBoundary}
              />
            </div>
            <HistoryPlugin />
            <ListPlugin />
            <CheckListPlugin />
            <LinkPlugin />
            <TabIndentationPlugin />
            <MarkdownShortcutPlugin transformers={TRANSFORMERS} />
            <OnChangePlugin
              ignoreSelectionChange
              onChange={(editorState) =>
                onChange(JSON.stringify(editorState.toJSON()))
              }
            />
          </div>
        </div>
      </div>
    </LexicalComposer>
  );
}
