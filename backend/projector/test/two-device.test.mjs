import { test } from "node:test";
import assert from "node:assert/strict";
import { createHeadlessEditor } from "@lexical/headless";
import { createBinding, syncLexicalUpdateToYjs, syncYjsChangesToLexical } from "@lexical/yjs";
import { $getRoot } from "lexical";
import * as Y from "yjs";
import { NODES, DOC_ID, NAMESPACE } from "../nodes.mjs";
import { jsonToUpdate } from "../seed.mjs";
import { updateToJson } from "../project.mjs";

// The two-device merge, headless. This mirrors the client wiring exactly:
// `useYjsCollaboration` skips syncing a Yjs transaction back to Lexical when its
// origin is the binding, and skips syncing a Lexical update to Yjs when it is
// tagged `skip-collab`. A client provider adds one more rule of its own — never
// re-append an update that arrived from the server — which is what REMOTE models.
const REMOTE = Symbol("remote");

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

function makeClient(seedUpdate) {
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
      if (!tags.has("skip-collab")) {
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
      }
    },
  );

  const outbox = [];
  doc.on("update", (update, origin) => {
    if (origin !== REMOTE) outbox.push(update);
  });

  Y.applyUpdate(doc, seedUpdate, REMOTE);
  editor.update(() => {}, { discrete: true });

  return {
    outbox,
    editText(index, next) {
      editor.update(
        () => {
          $getRoot().getChildren()[index].getFirstChild().setTextContent(next);
        },
        { discrete: true },
      );
    },
    receive(updates) {
      for (const update of updates) Y.applyUpdate(doc, update, REMOTE);
      editor.update(() => {}, { discrete: true });
    },
    text() {
      return editor.getEditorState().read(() => $getRoot().getTextContent());
    },
  };
}

test("concurrent edits to different paragraphs merge, losing neither", () => {
  const seed = jsonToUpdate(seedJson);

  const web = makeClient(seed);
  const desktop = makeClient(seed);

  // Both edit while partitioned — the desktop is offline.
  web.editText(0, "alpha edited by web");
  desktop.editText(1, "beta edited by desktop");

  const fromWeb = [...web.outbox];
  const fromDesktop = [...desktop.outbox];

  // Reconnect.
  desktop.receive(fromWeb);
  web.receive(fromDesktop);

  for (const client of [web, desktop]) {
    const text = client.text();
    assert.match(text, /alpha edited by web/);
    assert.match(text, /beta edited by desktop/);
  }

  // The server projects the same document from the append-only log.
  const projected = JSON.parse(updateToJson([seed, ...fromWeb, ...fromDesktop]));
  assert.equal(
    projected.root.children.length,
    2,
    "a merged note must not gain paragraphs",
  );
  assert.equal(projected.root.children[0].children[0].text, "alpha edited by web");
  assert.equal(projected.root.children[1].children[0].text, "beta edited by desktop");
});

test("updates received from the server are never re-appended", () => {
  const seed = jsonToUpdate(seedJson);

  const web = makeClient(seed);
  const desktop = makeClient(seed);

  // The seed arrived from the server; it must not be queued for upload.
  assert.equal(web.outbox.length, 0, "seed must not enter the outbox");

  desktop.editText(1, "beta edited by desktop");
  const fromDesktop = [...desktop.outbox];

  web.receive(fromDesktop);

  // Applying a remote update must not produce an update to send back. Without the
  // origin guard this echoes forever and the note's update log grows without end.
  assert.equal(web.outbox.length, 0, "remote updates must not echo back");
});

test("a client that edits the same paragraph concurrently keeps both clients converged", () => {
  const seed = jsonToUpdate(seedJson);

  const web = makeClient(seed);
  const desktop = makeClient(seed);

  web.editText(0, "written by web");
  desktop.editText(0, "written by desktop");

  const fromWeb = [...web.outbox];
  const fromDesktop = [...desktop.outbox];

  desktop.receive(fromWeb);
  web.receive(fromDesktop);

  // Yjs picks a winner deterministically; the requirement is that both clients and
  // the server agree on which, not which one wins.
  const projected = JSON.parse(updateToJson([seed, ...fromWeb, ...fromDesktop]));
  assert.equal(web.text(), desktop.text());
  assert.equal(
    JSON.parse(updateToJson([seed, ...fromDesktop, ...fromWeb])).root.children[0]
      .children[0].text,
    projected.root.children[0].children[0].text,
    "projection must not depend on the order updates were appended",
  );
});
