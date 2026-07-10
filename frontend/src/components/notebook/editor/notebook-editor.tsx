"use client";

import { DOC_ID, NAMESPACE } from "@tradstry/notebook-core";
import { CollaborationPlugin } from "@lexical/react/LexicalCollaborationPlugin";
import { LexicalCollaboration } from "@lexical/react/LexicalCollaborationContext";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { LinkPlugin } from "@lexical/react/LexicalLinkPlugin";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { MarkdownShortcutPlugin } from "@lexical/react/LexicalMarkdownShortcutPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { TabIndentationPlugin } from "@lexical/react/LexicalTabIndentationPlugin";
import { $getRoot, type EditorState } from "lexical";
import { useCallback, useEffect, useRef, useState } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { useGraphQL } from "@/lib/client";
import {
  fetchNotebookUpdates,
  type NotebookUpdate,
} from "@/lib/service/notebook-crdt";
import type { JournalEntry } from "@/lib/types/journal";
import type { NotebookImage } from "@/lib/types/notebook";
import { WEB_NODES } from "./nodes";
import type { Doc } from "yjs";
import { createGraphQLProvider } from "./yjs-provider";
import { LinkedTradeProvider } from "./nodes/linked-trade-node";
import { NotebookImageActionsProvider } from "./nodes/notebook-image-node";
import { AtMentionPlugin } from "./plugins/at-mention-plugin";
import { AutocompletePlugin } from "./plugins/autocomplete-plugin";
import { DraggableBlockPlugin } from "./plugins/draggable-block-plugin";
import { PasteImagePlugin } from "./plugins/paste-image-plugin";
import { SelectionToolbarPlugin } from "./plugins/selection-toolbar-plugin";
import { SlashCommandPlugin } from "./plugins/slash-command-plugin";
import { notebookEditorTheme } from "./theme";

const HEADER_PLACEHOLDER = "Header";
const BODY_PLACEHOLDER = "Start writing, or type / for commands.";

function PlaceholderPlugin() {
  const [editor] = useLexicalComposerContext();
  const [showHeaderPlaceholder, setShowHeaderPlaceholder] = useState(true);
  const [showBodyPlaceholder, setShowBodyPlaceholder] = useState(true);
  const [headerPlaceholderStyle, setHeaderPlaceholderStyle] = useState<{
    left: number;
    top: number;
  } | null>(null);
  const [bodyPlaceholderStyle, setBodyPlaceholderStyle] = useState<{
    left: number;
    top: number;
  } | null>(null);

  useEffect(() => {
    let resizeObserver: ResizeObserver | null = null;

    const updatePlaceholderState = () => {
      editor.getEditorState().read(() => {
        const root = $getRoot();
        const [headerNode, ...bodyNodes] = root.getChildren();
        const nextShowHeaderPlaceholder =
          headerNode === undefined ||
          headerNode.getTextContent().trim().length === 0;
        const nextShowBodyPlaceholder = bodyNodes.every(
          (node) => node.getTextContent().trim().length === 0,
        );

        setShowHeaderPlaceholder(nextShowHeaderPlaceholder);
        setShowBodyPlaceholder(nextShowBodyPlaceholder);
      });
    };

    const updatePlaceholderPosition = () => {
      const rootElement = editor.getRootElement();
      if (!rootElement) {
        return;
      }

      editor.getEditorState().read(() => {
        const root = $getRoot();
        const [headerNode, bodyNode] = root.getChildren();

        if (headerNode) {
          const headerElement = editor.getElementByKey(headerNode.getKey());
          if (headerElement) {
            const rootRect = rootElement.getBoundingClientRect();
            const headerRect = headerElement.getBoundingClientRect();

            setHeaderPlaceholderStyle({
              left: headerRect.left - rootRect.left,
              top: headerRect.top - rootRect.top,
            });
          } else {
            setHeaderPlaceholderStyle(null);
          }
        } else {
          setHeaderPlaceholderStyle(null);
        }

        if (bodyNode) {
          const bodyElement = editor.getElementByKey(bodyNode.getKey());
          if (bodyElement) {
            const rootRect = rootElement.getBoundingClientRect();
            const bodyRect = bodyElement.getBoundingClientRect();

            setBodyPlaceholderStyle({
              left: bodyRect.left - rootRect.left,
              top: bodyRect.top - rootRect.top,
            });
          } else {
            setBodyPlaceholderStyle(null);
          }
        } else {
          setBodyPlaceholderStyle(null);
        }
      });
    };

    updatePlaceholderState();
    updatePlaceholderPosition();

    const unregisterRootListener = editor.registerRootListener(
      (rootElement) => {
        if (resizeObserver) {
          resizeObserver.disconnect();
          resizeObserver = null;
        }

        if (!rootElement) {
          return;
        }

        resizeObserver = new ResizeObserver(() => {
          updatePlaceholderPosition();
        });

        resizeObserver.observe(rootElement);
      },
    );

    const unregisterUpdateListener = editor.registerUpdateListener(() => {
      updatePlaceholderState();
      updatePlaceholderPosition();
    });

    const handleWindowResize = () => {
      updatePlaceholderPosition();
    };

    window.addEventListener("resize", handleWindowResize);

    return () => {
      if (resizeObserver) {
        resizeObserver.disconnect();
      }
      unregisterRootListener();
      unregisterUpdateListener();
      window.removeEventListener("resize", handleWindowResize);
    };
  }, [editor]);

  return (
    <>
      {showHeaderPlaceholder && headerPlaceholderStyle ? (
        <div
          className="pointer-events-none absolute text-3xl font-semibold leading-tight tracking-tight text-muted-foreground"
          style={headerPlaceholderStyle}
        >
          {HEADER_PLACEHOLDER}
        </div>
      ) : null}
      {showBodyPlaceholder && bodyPlaceholderStyle ? (
        <div
          className="pointer-events-none absolute max-w-lg text-sm leading-7 text-muted-foreground"
          style={bodyPlaceholderStyle}
        >
          {BODY_PLACEHOLDER}
        </div>
      ) : null}
    </>
  );
}

type Loaded =
  | { kind: "loading" }
  | { kind: "pending" }
  | { kind: "crdt"; updates: NotebookUpdate[] }
  | { kind: "error" };

/**
 * Every note is a CRDT note: the device that creates one seeds it, and the server
 * seeds notes created through the web resolver. A note with no update blobs is one
 * whose seed has not arrived yet — not an old-style note. There is no
 * document-shaped write path left to fall back to.
 *
 * Mount with `key={noteId}` so switching notes rebuilds the Y.Doc.
 */
export function NotebookEditor({
  noteId,
  images = [],
  onUploadImage,
  onDeleteImage,
  trades = [],
  onLinkTrade,
  onUnlinkTrade,
}: {
  noteId: string;
  images?: NotebookImage[];
  onUploadImage?: (file: File, signal?: AbortSignal) => Promise<NotebookImage>;
  onDeleteImage?: (imageId: string) => Promise<void>;
  trades?: JournalEntry[];
  onLinkTrade?: (tradeId: string) => void;
  onUnlinkTrade?: (tradeId: string) => void;
}) {
  const fetcher = useGraphQL();
  const [state, setState] = useState<Loaded>({ kind: "loading" });
  const [floatingAnchorElem, setFloatingAnchorElem] =
    useState<HTMLDivElement | null>(null);

  const seedRef = useRef<NotebookUpdate[]>([]);
  if (state.kind === "crdt") seedRef.current = state.updates;
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  // `fetcher` must NOT be a dependency. It is memoized on Clerk's `getToken`, whose
  // identity churns; a new fetcher re-runs this effect, whose cleanup cancels the
  // in-flight request before it resolves — so `setState` never fires and the editor
  // loads forever. Every other caller passes `fetcher` into a queryFn, never a dep.
  useEffect(() => {
    let cancelled = false;
    fetchNotebookUpdates(fetcherRef.current, noteId, 0)
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

  // CollaborationPlugin memoizes the provider on the factory's identity. An inline
  // arrow is a new identity every render, so it rebuilt the Y.Doc, replayed every
  // update, and started another poll and flush interval on each one.
  const providerFactory = useCallback(
    (id: string, docMap: Map<string, Doc>) =>
      createGraphQLProvider(id, docMap, noteId, seedRef.current, fetcherRef.current),
    [noteId],
  );

  if (state.kind === "loading") {
    return (
      <div className="mx-auto w-full max-w-5xl px-4 sm:px-6 lg:px-10">
        <Skeleton
          data-testid="notebook-editor-loading"
          className="h-[42rem] rounded-[2rem]"
        />
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div className="mx-auto w-full max-w-5xl px-4 py-16 text-center text-sm text-muted-foreground sm:px-6 lg:px-10">
        Could not load this note. Reopen it to try again.
      </div>
    );
  }

  if (state.kind === "pending") {
    return (
      <div className="mx-auto w-full max-w-5xl px-4 py-16 text-center text-sm text-muted-foreground sm:px-6 lg:px-10">
        Syncing this note…
      </div>
    );
  }

  const editor = (
    <section className="mx-auto w-full max-w-5xl px-4 sm:px-6 lg:px-10">
      <LexicalComposer
        initialConfig={{
          namespace: NAMESPACE,
          theme: notebookEditorTheme,
          // Collaboration owns the state; a seeded editorState would double it up.
          editorState: null,
          nodes: WEB_NODES,
          onError(error) {
            throw error;
          },
        }}
      >
        <NotebookImageActionsProvider images={images} onDeleteImage={onDeleteImage}>
          <LinkedTradeProvider trades={trades} onUnlinkTrade={onUnlinkTrade}>
            {/* shouldBootstrap MUST stay false: the client never seeds a Y.Doc.
                Two independently-bootstrapped docs concatenate rather than merge,
                silently duplicating every paragraph. Only the creator seeds. */}
            <CollaborationPlugin
              id={DOC_ID}
              providerFactory={providerFactory}
              shouldBootstrap={false}
            />
            <div
              ref={(el) => {
                if (el !== null) {
                  setFloatingAnchorElem(el);
                }
              }}
              className="relative min-h-[42rem]"
            >
              <RichTextPlugin
                contentEditable={
                  <ContentEditable className="min-h-[42rem] resize-none py-2 pr-1 pl-12 text-[15px] leading-7 text-foreground outline-none" />
                }
                placeholder={null}
                ErrorBoundary={LexicalErrorBoundary}
              />
              <PlaceholderPlugin />
              <ListPlugin />
              <LinkPlugin />
              <TabIndentationPlugin />
              <MarkdownShortcutPlugin />
              <SlashCommandPlugin trades={trades} onLinkTrade={onLinkTrade} />
              <AtMentionPlugin trades={trades} onLinkTrade={onLinkTrade} />
              <PasteImagePlugin onUploadImage={onUploadImage} />
              <AutocompletePlugin fetcher={fetcher} />
              <SelectionToolbarPlugin fetcher={fetcher} />
              {floatingAnchorElem ? (
                <DraggableBlockPlugin anchorElem={floatingAnchorElem} />
              ) : null}
            </div>
          </LinkedTradeProvider>
        </NotebookImageActionsProvider>
      </LexicalComposer>
    </section>
  );

  return <LexicalCollaboration>{editor}</LexicalCollaboration>;
}
