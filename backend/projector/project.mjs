import { createHeadlessEditor } from "@lexical/headless";
import { createBinding, syncYjsChangesToLexical } from "@lexical/yjs";
import * as Y from "yjs";
import { NODES, stripGhostText, DOC_ID, NAMESPACE } from "./nodes.mjs";

const provider = { awareness: { getLocalState: () => null, getStates: () => [] } };

export function updateToJson(updates) {
  // An empty projection would wipe the note's title and its embeddings. A note with
  // zero updates is never projectable — fail rather than emit an empty document.
  if (!updates || updates.length === 0) {
    throw new Error("updateToJson: refusing to project a note with zero updates");
  }

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

  binding.root
    .getSharedType()
    .observeDeep((events) =>
      syncYjsChangesToLexical(binding, provider, events, false),
    );

  for (const update of updates) Y.applyUpdate(doc, update);

  // Not optional: without a discrete update the editor state stays EMPTY despite
  // the observeDeep sync having fired.
  editor.update(() => {}, { discrete: true });

  return JSON.stringify(stripGhostText(editor.getEditorState().toJSON()));
}

async function main() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const raw = Buffer.concat(chunks).toString("utf8");

  const updates = raw
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => new Uint8Array(Buffer.from(line, "base64")));

  process.stdout.write(updateToJson(updates));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  // Never emit an empty document on failure: a blank projection would silently
  // wipe the note's derived title and its vector-search embeddings.
  main().catch((e) => {
    process.stderr.write(String(e?.stack ?? e) + "\n");
    process.exit(1);
  });
}
