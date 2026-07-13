import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { CheckListPlugin } from "@lexical/react/LexicalCheckListPlugin";
import { LinkPlugin } from "@lexical/react/LexicalLinkPlugin";
import { TabIndentationPlugin } from "@lexical/react/LexicalTabIndentationPlugin";
import { MarkdownShortcutPlugin } from "@lexical/react/LexicalMarkdownShortcutPlugin";
import { OnChangePlugin } from "@lexical/react/LexicalOnChangePlugin";
import { CollaborationPlugin } from "@lexical/react/LexicalCollaborationPlugin";
import { LexicalCollaboration } from "@lexical/react/LexicalCollaborationContext";
import { HorizontalRulePlugin } from "@lexical/react/LexicalHorizontalRulePlugin";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { TRANSFORMERS } from "@lexical/markdown";
import { DOC_ID, NAMESPACE } from "@tradstry/notebook-core";
import type { Doc } from "yjs";
import { DESKTOP_NODES } from "./notebook-nodes";
import { editorTheme } from "./editor-theme";
import { Toolbar } from "./toolbar";
import { SlashMenuPlugin } from "./slash-menu";
import { CodeHighlightPlugin, CodeLanguagePlugin } from "./code-block";
import { createLocalProvider } from "./yjs-provider";
import { MediaResolverProvider } from "./media-resolver";
import { PasteMediaPlugin } from "./paste-media-plugin";
import { cacheNoteBody, noteUpdates } from "../../../backend";

const onError = (error: Error) => console.error(error);

function EditorSurface({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <Toolbar />
      <div className="min-h-0 flex-1 overflow-auto">
        <div className="mx-auto max-w-4xl px-8 py-6">
          <div className="relative">
            <RichTextPlugin
              contentEditable={
                <ContentEditable
                  spellCheck
                  className="min-h-[55vh] text-[15px] leading-7 text-zinc-700 outline-none dark:text-zinc-200"
                />
              }
              placeholder={
                <div className="pointer-events-none absolute left-0 top-0 text-[15px] leading-7 text-zinc-400 dark:text-zinc-600">
                  Start writing… try “# ” for a heading or “- ” for a list.
                </div>
              }
              ErrorBoundary={LexicalErrorBoundary}
            />
            <CodeLanguagePlugin />
          </div>
          <ListPlugin />
          <CheckListPlugin />
          <LinkPlugin />
          <HorizontalRulePlugin />
          <CodeHighlightPlugin />
          <SlashMenuPlugin />
          <TabIndentationPlugin />
          <MarkdownShortcutPlugin transformers={TRANSFORMERS} />
          {children}
        </div>
      </div>
    </div>
  );
}

const CACHE_DEBOUNCE_MS = 500;

/**
 * Keeps the note list's title and preview current. The body is a CRDT and syncs as
 * update blobs; this only refreshes the local projection of it, which the server
 * re-derives independently.
 */
function CacheProjectionPlugin({
  noteId,
  onCached,
}: {
  noteId: string;
  onCached?: () => void;
}) {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  return (
    <OnChangePlugin
      ignoreSelectionChange
      onChange={(editorState) => {
        if (timer.current) clearTimeout(timer.current);
        const json = JSON.stringify(editorState.toJSON());
        timer.current = setTimeout(() => {
          void cacheNoteBody(noteId, json).then(() => onCached?.());
        }, CACHE_DEBOUNCE_MS);
      }}
    />
  );
}

function CrdtEditor({
  noteId,
  accountId,
  seedUpdatesB64,
  onBodyCached,
}: {
  noteId: string;
  accountId: string;
  seedUpdatesB64: string[];
  onBodyCached?: () => void;
}) {
  const providerFactory = useCallback(
    (id: string, docMap: Map<string, Doc>) =>
      createLocalProvider(id, docMap, noteId, seedUpdatesB64),
    [noteId, seedUpdatesB64],
  );

  return (
    <LexicalCollaboration>
      <LexicalComposer
        initialConfig={{
          namespace: NAMESPACE,
          theme: editorTheme,
          // Collaboration owns the state; a seeded editorState would double it up.
          editorState: null,
          onError,
          nodes: [...DESKTOP_NODES],
        }}
      >
        <MediaResolverProvider noteId={noteId}>
          <EditorSurface>
            {/* shouldBootstrap MUST stay false: the client never seeds a Y.Doc.
                Two independently-bootstrapped docs concatenate rather than merge,
                silently duplicating every paragraph. Only the server seeds. */}
            <CollaborationPlugin
              id={DOC_ID}
              providerFactory={providerFactory}
              shouldBootstrap={false}
            />
            <CacheProjectionPlugin noteId={noteId} onCached={onBodyCached} />
            <PasteMediaPlugin noteId={noteId} accountId={accountId} />
          </EditorSurface>
        </MediaResolverProvider>
      </LexicalComposer>
    </LexicalCollaboration>
  );
}

type Loaded =
  | { kind: "loading" }
  | { kind: "pending" }
  | { kind: "crdt"; updates: string[] }
  | { kind: "error" };

/**
 * Every note is a CRDT note: the device that creates one seeds it, and the sync
 * loop pulls the seed for notes created elsewhere. A note with no update blobs is
 * therefore one whose seed has not arrived yet, not an old-style note — there is no
 * document-shaped write path left to fall back to. Mount with `key={note.id}` so
 * switching notes reloads the Y.Doc.
 */
export function NotebookEditor({
  noteId,
  accountId,
  onBodyCached,
}: {
  noteId: string;
  accountId: string;
  onBodyCached?: () => void;
}) {
  const [state, setState] = useState<Loaded>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    noteUpdates(noteId)
      .then((updates) => {
        if (cancelled) return;
        setState(updates.length > 0 ? { kind: "crdt", updates } : { kind: "pending" });
      })
      .catch(() => {
        if (!cancelled) setState({ kind: "error" });
      });
    return () => {
      cancelled = true;
    };
  }, [noteId]);

  // A full-height shell keyed on the note so the layout never collapses between
  // notes — that reflow is what made the toolbar jump. AnimatePresence cross-fades
  // the outgoing and incoming note; `mode="popLayout"` keeps them from stacking and
  // pushing the toolbar. `loading` renders an empty shell (blobs are on-device and
  // resolve within a frame), so there is no spinner flash on every switch.
  return (
    <AnimatePresence mode="popLayout" initial={false}>
      <motion.div
        key={state.kind === "crdt" ? noteId : state.kind}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.15, ease: "easeOut" }}
        className="flex h-full min-h-0 flex-1 flex-col"
      >
        {state.kind === "crdt" ? (
          <CrdtEditor
            noteId={noteId}
            accountId={accountId}
            seedUpdatesB64={state.updates}
            onBodyCached={onBodyCached}
          />
        ) : (
          <div className="flex flex-1 items-center justify-center p-6 text-center text-sm text-zinc-500 dark:text-zinc-400">
            {state.kind === "error"
              ? "Could not load this note. Reopen it to try again."
              : state.kind === "pending"
                ? "Syncing this note…"
                : null}
          </div>
        )}
      </motion.div>
    </AnimatePresence>
  );
}
