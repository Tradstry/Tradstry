import { createHeadlessEditor } from "@lexical/headless";
import { createBinding, syncLexicalUpdateToYjs } from "@lexical/yjs";
import * as Y from "yjs";
import { DOC_ID, NAMESPACE } from "./contract";
import { NODES } from "./nodes";

const provider = {
  awareness: { getLocalState: () => null, getStates: () => [] },
} as never;

export type Seed = {
  update: Uint8Array;
  stateVector: Uint8Array;
};

/**
 * Builds a note's first Yjs update from its `document_json`.
 *
 * A note must be seeded exactly once, by exactly one place: two independently
 * seeded Y.Docs do not conflict, they concatenate — silently duplicating every
 * paragraph. Whoever creates a note seeds it, and this is the only implementation
 * of that step, shared by the desktop and the server-side projector so the two can
 * never produce different bytes for the same document.
 */
export function jsonToUpdate(documentJson: string): Seed {
  const editor = createHeadlessEditor({
    namespace: NAMESPACE,
    nodes: NODES,
    onError: (e) => {
      throw e;
    },
  });

  const doc = new Y.Doc();
  const binding = createBinding(
    editor,
    provider,
    DOC_ID,
    doc,
    new Map([[DOC_ID, doc]]),
  );

  // The listener must exist before the state is set, or the initial content never
  // reaches the Y.Doc and the seed is silently empty.
  editor.registerUpdateListener(
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

  editor.setEditorState(editor.parseEditorState(documentJson));

  // Without a discrete update the listener never fires and the Y.Doc stays empty.
  editor.update(() => {}, { discrete: true });

  const update = Y.encodeStateAsUpdate(doc);
  return { update, stateVector: Y.encodeStateVectorFromUpdate(update) };
}
