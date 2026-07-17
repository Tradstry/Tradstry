import { createHeadlessEditor } from "@lexical/headless";
import { $convertFromMarkdownString, TRANSFORMERS } from "@lexical/markdown";
import {
  createBinding,
  syncLexicalUpdateToYjs,
  syncYjsChangesToLexical,
} from "@lexical/yjs";
import { $getRoot, $parseSerializedNode } from "lexical";
import * as Y from "yjs";
import { DOC_ID, NAMESPACE, NODES } from "./nodes.mjs";

const provider = {
  awareness: { getLocalState: () => null, getStates: () => [] },
};

function editor() {
  return createHeadlessEditor({
    namespace: NAMESPACE,
    nodes: NODES,
    onError: (e) => {
      throw e;
    },
  });
}

/** Markdown -> Lexical `document_json`, for a note that does not exist yet. */
export function markdownToJson(markdown) {
  const ed = editor();
  ed.update(
    () => {
      $convertFromMarkdownString(markdown, TRANSFORMERS);
    },
    { discrete: true },
  );
  return JSON.stringify(ed.getEditorState().toJSON());
}

/**
 * The markdown's nodes, detached from any editor, so they can be appended to a
 * document that already exists rather than replacing it.
 */
function markdownNodes(markdown) {
  const ed = editor();
  ed.update(
    () => {
      $convertFromMarkdownString(markdown, TRANSFORMERS);
    },
    { discrete: true },
  );
  return ed.getEditorState().toJSON().root.children;
}

/**
 * Appends markdown to an existing `document_json`, for a note no client has opened yet.
 *
 * Structural, not a markdown round-trip: rendering the existing body to markdown and back
 * would silently drop every node markdown cannot express — images, linked trades — so the
 * note's own content is parsed as nodes and the new ones appended after it.
 */
export function appendToJson(documentJson, markdown) {
  const ed = editor();
  ed.setEditorState(ed.parseEditorState(documentJson));
  const children = markdownNodes(markdown);
  ed.update(
    () => {
      const root = $getRoot();
      for (const child of children) root.append($parseSerializedNode(child));
    },
    { discrete: true },
  );
  return JSON.stringify(ed.getEditorState().toJSON());
}

/**
 * Edits a note that is already CRDT-backed, returning ONLY the incremental Yjs update.
 *
 * The update must be a delta against the note's existing history — a doc rebuilt from
 * scratch would not conflict with the live document, it would concatenate with it and
 * silently duplicate every paragraph. So: replay the history, start recording, mutate
 * through the Lexical binding, and hand back just what that mutation produced. Anything
 * the user is concurrently typing then merges with it instead of being clobbered.
 */
export function applyMarkdown(updates, markdown, mode) {
  if (!updates || updates.length === 0) {
    throw new Error("applyMarkdown: note has no updates to edit");
  }

  const ed = editor();
  const doc = new Y.Doc();
  const binding = createBinding(
    ed,
    provider,
    DOC_ID,
    doc,
    new Map([[DOC_ID, doc]]),
  );

  binding.root
    .getSharedType()
    .observeDeep((events) =>
      syncYjsChangesToLexical(binding, provider, events, false),
    );

  for (const update of updates) Y.applyUpdate(doc, update);
  // Without a discrete pass the editor state stays empty despite the Y.Doc being full.
  ed.update(() => {}, { discrete: true });

  // Record from here: everything before this point is history we must not re-emit.
  const produced = [];
  doc.on("update", (bytes) => produced.push(bytes));

  // Registered only now, after hydration: attached earlier, replaying the history into
  // Lexical would echo straight back into the Y.Doc and duplicate the document.
  ed.registerUpdateListener(
    ({
      prevEditorState,
      editorState,
      dirtyElements,
      dirtyLeaves,
      normalizedNodes,
      tags,
    }) => {
      syncLexicalUpdateToYjs(
        binding,
        provider,
        prevEditorState,
        editorState,
        dirtyElements,
        dirtyLeaves,
        normalizedNodes,
        tags,
      );
    },
  );

  const children = markdownNodes(markdown);
  ed.update(
    () => {
      const root = $getRoot();
      if (mode === "replace") root.clear();
      for (const child of children) root.append($parseSerializedNode(child));
    },
    { discrete: true },
  );

  if (produced.length === 0) {
    throw new Error("applyMarkdown: edit produced no update");
  }
  return Y.mergeUpdates(produced);
}

async function main() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const input = JSON.parse(Buffer.concat(chunks).toString("utf8"));

  if (input.op === "toJson") {
    process.stdout.write(
      JSON.stringify({ documentJson: markdownToJson(input.markdown) }),
    );
    return;
  }

  if (input.op === "appendJson") {
    process.stdout.write(
      JSON.stringify({
        documentJson: appendToJson(input.documentJson, input.markdown),
      }),
    );
    return;
  }

  if (input.op === "apply") {
    const updates = (input.updates ?? []).map((u) =>
      new Uint8Array(Buffer.from(u, "base64")),
    );
    const update = applyMarkdown(updates, input.markdown, input.mode);
    process.stdout.write(
      JSON.stringify({ update: Buffer.from(update).toString("base64") }),
    );
    return;
  }

  throw new Error(`markdown.mjs: unknown op ${input.op}`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => {
    process.stderr.write(String(e?.stack ?? e) + "\n");
    process.exit(1);
  });
}
