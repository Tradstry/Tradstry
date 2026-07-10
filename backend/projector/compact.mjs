import * as Y from "yjs";

/**
 * Collapses an update chain into one equivalent blob. Pure Yjs — no Lexical, no
 * node registry — so a compacted note is byte-for-byte projectable by the same
 * `updateToJson` that produced its history.
 */
export function mergeUpdateChain(updates) {
  // Merging nothing yields an empty blob, which would replace a note's entire
  // history with a document that projects as empty.
  if (!updates || updates.length === 0) {
    throw new Error("mergeUpdateChain: refusing to compact a note with zero updates");
  }
  return Y.mergeUpdates(updates);
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

  process.stdout.write(Buffer.from(mergeUpdateChain(updates)).toString("base64"));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => {
    process.stderr.write(String(e?.stack ?? e) + "\n");
    process.exit(1);
  });
}
