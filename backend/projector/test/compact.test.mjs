import { test } from "node:test";
import assert from "node:assert/strict";
import { createHeadlessEditor } from "@lexical/headless";
import { createBinding, syncLexicalUpdateToYjs, syncYjsChangesToLexical } from "@lexical/yjs";
import { $getRoot } from "lexical";
import * as Y from "yjs";
import { NODES, DOC_ID, NAMESPACE } from "../nodes.mjs";
import { mergeUpdateChain } from "../compact.mjs";
import { jsonToUpdate } from "../seed.mjs";
import { updateToJson } from "../project.mjs";

const provider = { awareness: { getLocalState: () => null, getStates: () => [] } };

const paragraph = (t) => ({
  type: "paragraph",
  version: 1,
  direction: "ltr",
  format: "",
  indent: 0,
  children: [
    { type: "text", text: t, format: 0, detail: 0, mode: "normal", style: "", version: 1 },
  ],
});

const seedJson = JSON.stringify({
  root: {
    type: "root",
    version: 1,
    direction: "ltr",
    format: "",
    indent: 0,
    children: [paragraph("alpha"), paragraph("beta")],
  },
});

/**
 * A seed followed by real document-changing deltas — the shape of a note's chain.
 * The edits must alter the projected text, or a merge that silently dropped them
 * would still project identically and the test would prove nothing.
 */
function chainOf(edits) {
  const seed = jsonToUpdate(seedJson);

  const editor = createHeadlessEditor({
    namespace: NAMESPACE,
    nodes: NODES,
    onError: (e) => {
      throw e;
    },
  });
  const doc = new Y.Doc();
  const binding = createBinding(editor, provider, DOC_ID, doc, new Map([[DOC_ID, doc]]));

  binding.root.getSharedType().observeDeep((events, tx) => {
    if (tx.origin !== binding) syncYjsChangesToLexical(binding, provider, events, false);
  });

  editor.registerUpdateListener(
    ({ prevEditorState, editorState, dirtyElements, dirtyLeaves, normalizedNodes, tags }) => {
      if (tags.has("skip-collab")) return;
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

  const chain = [seed];
  Y.applyUpdate(doc, seed);
  editor.update(() => {}, { discrete: true });
  doc.on("update", (update) => chain.push(update));

  for (let i = 0; i < edits; i += 1) {
    editor.update(
      () => {
        $getRoot().getChildren()[0].getFirstChild().setTextContent(`alpha v${i}`);
      },
      { discrete: true },
    );
  }
  return chain;
}

test("a compacted chain projects identically to the chain it replaced", () => {
  const chain = chainOf(5);
  assert.ok(chain.length > 1, "the chain must contain real deltas");

  const projectedBefore = updateToJson(chain);
  assert.match(projectedBefore, /alpha v4/, "the deltas must change the document");

  assert.equal(updateToJson([mergeUpdateChain(chain)]), projectedBefore);
});

test("dropping a delta from the merge changes the projection", () => {
  const chain = chainOf(5);
  // Guards the test above: if the merge silently lost the last edit, this is what
  // that failure would look like. It must not project the same.
  assert.notEqual(
    updateToJson([mergeUpdateChain(chain.slice(0, -1))]),
    updateToJson(chain),
  );
});

test("a delta that lands after the snapshot still applies to the merged blob", () => {
  const chain = chainOf(2);
  const late = chain.pop();
  const merged = mergeUpdateChain(chain);

  // compact_note deletes rows <= max_seq and appends the merge at a *higher* seq,
  // so a concurrent append ends up before the merged blob in seq order. Yjs updates
  // commute; the projection must not care which order the log holds them in.
  assert.equal(updateToJson([late, merged]), updateToJson([merged, late]));
});

test("merging an empty chain is refused rather than silently emptying the note", () => {
  assert.throws(() => mergeUpdateChain([]), /zero updates/);
});
