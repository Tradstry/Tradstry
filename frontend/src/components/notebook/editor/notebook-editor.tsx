"use client";

import { CodeNode } from "@lexical/code";
import { LinkNode } from "@lexical/link";
import { ListItemNode, ListNode } from "@lexical/list";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { HorizontalRuleNode } from "@lexical/react/LexicalHorizontalRuleNode";
import { LinkPlugin } from "@lexical/react/LexicalLinkPlugin";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { MarkdownShortcutPlugin } from "@lexical/react/LexicalMarkdownShortcutPlugin";
import { OnChangePlugin } from "@lexical/react/LexicalOnChangePlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { TabIndentationPlugin } from "@lexical/react/LexicalTabIndentationPlugin";
import {
  $createHeadingNode,
  $isHeadingNode,
  HeadingNode,
  QuoteNode,
} from "@lexical/rich-text";
import {
  $createParagraphNode,
  $getRoot,
  $getSelection,
  $isRangeSelection,
  type EditorState,
  type LexicalEditor,
} from "lexical";
import { useEffect, useRef, useState } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { useGraphQL } from "@/lib/client";
import type { JournalEntry } from "@/lib/types/journal";
import type { NotebookImage } from "@/lib/types/notebook";
import { GhostTextNode } from "./nodes/ghost-text-node";
import {
  LinkedTradeNode,
  LinkedTradeProvider,
} from "./nodes/linked-trade-node";
import {
  NotebookImageActionsProvider,
  NotebookImageNode,
} from "./nodes/notebook-image-node";
import { AtMentionPlugin } from "./plugins/at-mention-plugin";
import { AutocompletePlugin } from "./plugins/autocomplete-plugin";
import { PasteImagePlugin } from "./plugins/paste-image-plugin";
import { SelectionToolbarPlugin } from "./plugins/selection-toolbar-plugin";
import { SlashCommandPlugin } from "./plugins/slash-command-plugin";
import { notebookEditorTheme } from "./theme";

const NOTEBOOK_STORAGE_KEY = "tradstry-notebook-editor-state";
const HEADER_PLACEHOLDER = "Header";
const BODY_PLACEHOLDER = "Start writing, or type / for commands.";
const NOTEBOOK_EDITOR_COMPOSER_KEY = `TradstryNotebookEditor:${Math.random()
  .toString(36)
  .slice(2)}`;

export function createDefaultNotebookDocumentJson(): string {
  return JSON.stringify({
    root: {
      children: [
        {
          children: [],
          direction: null,
          format: "",
          indent: 0,
          type: "heading",
          version: 1,
          tag: "h1",
        },
        {
          children: [],
          direction: null,
          format: "",
          indent: 0,
          type: "paragraph",
          version: 1,
        },
      ],
      direction: null,
      format: "",
      indent: 0,
      type: "root",
      version: 1,
    },
    selection: null,
  });
}

export function normalizeNotebookDocumentJson(
  serializedEditorState: string | null,
): string | null {
  if (!serializedEditorState) {
    return null;
  }

  try {
    const parsedState = JSON.parse(serializedEditorState) as {
      root?: unknown;
      selection?: unknown;
    };

    if (!parsedState.root) {
      return null;
    }

    return JSON.stringify({
      ...parsedState,
      selection: null,
    });
  } catch {
    return null;
  }
}

function visitNotebookDocumentNodes(
  nodes: unknown,
  visitor: (node: Record<string, unknown>) => void,
) {
  if (!Array.isArray(nodes)) {
    return;
  }

  for (const child of nodes) {
    if (!child || typeof child !== "object") {
      continue;
    }

    const node = child as Record<string, unknown>;
    visitor(node);
    visitNotebookDocumentNodes(node.children, visitor);
  }
}

export function mergeNotebookImagesIntoDocumentJson(
  documentJson: string | null,
  images: NotebookImage[],
): string | null {
  const normalizedDocumentJson = normalizeNotebookDocumentJson(documentJson);
  if (!normalizedDocumentJson || images.length === 0) {
    return normalizedDocumentJson;
  }

  try {
    const parsed = JSON.parse(normalizedDocumentJson) as {
      root?: { children?: unknown };
      selection?: unknown;
    };
    const imagesById = new Map(images.map((image) => [image.id, image]));

    visitNotebookDocumentNodes(parsed.root?.children, (node) => {
      if (node.type !== "notebook-image") {
        return;
      }

      const imageId = node.imageId;
      if (typeof imageId !== "string") {
        return;
      }

      const image = imagesById.get(imageId);
      if (!image) {
        return;
      }

      node.src = image.secureUrl;
      node.width = image.width;
      node.height = image.height;

      if (
        (typeof node.altText !== "string" || node.altText.length === 0) &&
        image.originalFilename
      ) {
        node.altText = image.originalFilename;
      }
    });

    return JSON.stringify({
      ...parsed,
      selection: null,
    });
  } catch {
    return normalizedDocumentJson;
  }
}

function HydrationPlugin({
  initialEditorState,
}: {
  initialEditorState: string | null;
}) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    if (!initialEditorState) {
      return;
    }

    try {
      const parsedEditorState = editor.parseEditorState(initialEditorState);
      editor.setEditorState(parsedEditorState);
      editor.update(() => {
        ensureNotebookStructure();
      });
    } catch {
      window.localStorage.removeItem(NOTEBOOK_STORAGE_KEY);
    }
  }, [editor, initialEditorState]);

  return null;
}

function ensureNotebookStructure() {
  const root = $getRoot();
  let firstChild = root.getFirstChild();

  if (firstChild === null) {
    root.append($createHeadingNode("h1"), $createParagraphNode());
    return;
  }

  if (!$isHeadingNode(firstChild)) {
    if (
      root.getChildrenSize() === 1 &&
      firstChild.getTextContent().trim().length === 0
    ) {
      firstChild.replace($createHeadingNode("h1"));
    } else {
      firstChild.insertBefore($createHeadingNode("h1"));
    }
    firstChild = root.getFirstChild();
  }

  if ($isHeadingNode(firstChild) && firstChild.getTag() !== "h1") {
    firstChild.setTag("h1");
  }

  if (root.getChildrenSize() === 1) {
    root.append($createParagraphNode());
  }
}

function TitleBehaviorPlugin() {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    let removeFocusListener: (() => void) | null = null;

    return editor.registerRootListener((rootElement, previousRootElement) => {
      if (previousRootElement && removeFocusListener) {
        removeFocusListener();
        removeFocusListener = null;
      }

      if (!rootElement) {
        return;
      }

      const handleFocus = () => {
        editor.update(() => {
          ensureNotebookStructure();

          const root = $getRoot();
          const [headerNode, ...bodyNodes] = root.getChildren();
          if (!$isHeadingNode(headerNode) || headerNode.getTag() !== "h1") {
            return;
          }

          const headerEmpty = headerNode.getTextContent().trim().length === 0;
          const bodyEmpty = bodyNodes.every(
            (node) => node.getTextContent().trim().length === 0,
          );
          const selection = $getSelection();
          const selectionInHeader =
            $isRangeSelection(selection) &&
            selection.anchor
              .getNode()
              .getTopLevelElementOrThrow()
              .is(headerNode);

          if (headerEmpty && bodyEmpty && !selectionInHeader) {
            headerNode.selectStart();
          }
        });
      };

      rootElement.addEventListener("focusin", handleFocus);
      removeFocusListener = () => {
        rootElement.removeEventListener("focusin", handleFocus);
      };
    });
  }, [editor]);

  return null;
}

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
          className="pointer-events-none absolute text-3xl font-semibold leading-tight tracking-tight text-slate-300"
          style={headerPlaceholderStyle}
        >
          {HEADER_PLACEHOLDER}
        </div>
      ) : null}
      {showBodyPlaceholder && bodyPlaceholderStyle ? (
        <div
          className="pointer-events-none absolute max-w-lg text-sm leading-7 text-slate-400"
          style={bodyPlaceholderStyle}
        >
          {BODY_PLACEHOLDER}
        </div>
      ) : null}
    </>
  );
}

function PersistencePlugin({
  storageKey,
  onSerializedChange,
}: {
  storageKey: string;
  onSerializedChange?: ((serializedEditorState: string) => void) | undefined;
}) {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const handleChange = (editorState: EditorState, _editor: LexicalEditor) => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }

    timeoutRef.current = setTimeout(() => {
      let serializedEditorState = JSON.stringify({
        ...editorState.toJSON(),
        selection: null,
      });

      // Don't persist while any image is still a temp blob URL —
      // blob URLs are revoked after upload and would cause broken images on reload
      if (serializedEditorState.includes("blob:")) return;

      // Strip ghost-text nodes before persisting
      if (serializedEditorState.includes('"ghost-text"')) {
        try {
          const parsed = JSON.parse(serializedEditorState);
          const stripGhost = (nodes: any[]): any[] =>
            nodes
              .filter((n: any) => n.type !== "ghost-text")
              .map((n: any) => ({
                ...n,
                children: n.children ? stripGhost(n.children) : n.children,
              }));
          if (parsed.root?.children) {
            parsed.root.children = stripGhost(parsed.root.children);
          }
          serializedEditorState = JSON.stringify(parsed);
        } catch {
          /* ignore */
        }
      }

      window.localStorage.setItem(storageKey, serializedEditorState);
      onSerializedChange?.(serializedEditorState);
    }, 300);
  };

  return <OnChangePlugin ignoreSelectionChange onChange={handleChange} />;
}

export function NotebookEditor({
  initialDocumentJson = null,
  draftStorageKey = NOTEBOOK_STORAGE_KEY,
  onSerializedChange,
  onUploadImage,
  onDeleteImage,
  trades = [],
  onLinkTrade,
  onUnlinkTrade,
}: {
  initialDocumentJson?: string | null;
  draftStorageKey?: string;
  onSerializedChange?: (serializedEditorState: string) => void;
  onUploadImage?: (file: File) => Promise<NotebookImage>;
  onDeleteImage?: (imageId: string) => Promise<void>;
  trades?: JournalEntry[];
  onLinkTrade?: (tradeId: string) => void;
  onUnlinkTrade?: (tradeId: string) => void;
}) {
  const fetcher = useGraphQL();
  const [initialEditorState, setInitialEditorState] = useState<string | null>(
    null,
  );
  const [isReady, setIsReady] = useState(false);
  // Mount-once hydration guard. We deliberately ignore subsequent
  // `initialDocumentJson` prop changes within the same note — the parent
  // remounts the editor (via `key={selectedNote.id}`) when switching notes,
  // so the only legitimate hydration is the initial one. Re-running this on
  // prop changes (e.g. from a refetch landing while the user is typing)
  // would call HydrationPlugin.setEditorState and wipe in-flight keystrokes.
  const didHydrateRef = useRef(false);

  useEffect(() => {
    if (didHydrateRef.current) return;
    didHydrateRef.current = true;

    const storedEditorState = normalizeNotebookDocumentJson(
      window.localStorage.getItem(draftStorageKey),
    );
    const fallbackEditorState =
      normalizeNotebookDocumentJson(initialDocumentJson);

    if (!storedEditorState && !fallbackEditorState) {
      window.localStorage.removeItem(draftStorageKey);
    }

    if (!storedEditorState && fallbackEditorState) {
      window.localStorage.setItem(draftStorageKey, fallbackEditorState);
    }

    setInitialEditorState(storedEditorState ?? fallbackEditorState);
    setIsReady(true);
  }, [draftStorageKey, initialDocumentJson]);

  if (!isReady) {
    return (
      <div className="mx-auto w-full max-w-5xl space-y-4 px-4 sm:px-6 lg:px-10">
        <Skeleton className="h-[42rem] rounded-[2rem]" />
      </div>
    );
  }

  return (
    <section className="mx-auto w-full max-w-5xl px-4 sm:px-6 lg:px-10">
      <LexicalComposer
        key={NOTEBOOK_EDITOR_COMPOSER_KEY}
        initialConfig={{
          namespace: "TradstryNotebookEditor",
          theme: notebookEditorTheme,
          editorState: () => {
            ensureNotebookStructure();
          },
          nodes: [
            HeadingNode,
            QuoteNode,
            ListNode,
            ListItemNode,
            LinkNode,
            CodeNode,
            HorizontalRuleNode,
            NotebookImageNode,
            GhostTextNode,
            LinkedTradeNode,
          ],
          onError(error) {
            throw error;
          },
        }}
      >
        <NotebookImageActionsProvider onDeleteImage={onDeleteImage}>
          <LinkedTradeProvider trades={trades} onUnlinkTrade={onUnlinkTrade}>
            <HydrationPlugin initialEditorState={initialEditorState} />
            <TitleBehaviorPlugin />
            <div className="relative min-h-[42rem]">
              <RichTextPlugin
                contentEditable={
                  <ContentEditable className="min-h-[42rem] resize-none px-1 py-2 text-[15px] leading-7 text-foreground outline-none" />
                }
                placeholder={null}
                ErrorBoundary={LexicalErrorBoundary}
              />
              <PlaceholderPlugin />
              <HistoryPlugin />
              <ListPlugin />
              <LinkPlugin />
              <TabIndentationPlugin />
              <MarkdownShortcutPlugin />
              <SlashCommandPlugin trades={trades} onLinkTrade={onLinkTrade} />
              <AtMentionPlugin trades={trades} onLinkTrade={onLinkTrade} />
              <PasteImagePlugin onUploadImage={onUploadImage} />
              <AutocompletePlugin fetcher={fetcher} />
              <SelectionToolbarPlugin fetcher={fetcher} />
              <PersistencePlugin
                storageKey={draftStorageKey}
                onSerializedChange={onSerializedChange}
              />
            </div>
          </LinkedTradeProvider>
        </NotebookImageActionsProvider>
      </LexicalComposer>
    </section>
  );
}
