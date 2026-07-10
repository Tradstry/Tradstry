import { jsonToUpdate as buildSeed } from "./notebook-seed.gen.mjs";

/**
 * Seeding lives in @tradstry/notebook-core so the desktop — which seeds the notes
 * it creates offline — and this server-side path cannot produce different bytes
 * for the same document.
 */
export function jsonToUpdate(documentJson) {
  return buildSeed(documentJson).update;
}

async function main() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const documentJson = Buffer.concat(chunks).toString("utf8");

  const { update, stateVector } = buildSeed(documentJson);
  process.stdout.write(
    JSON.stringify({
      update: Buffer.from(update).toString("base64"),
      stateVector: Buffer.from(stateVector).toString("base64"),
    }),
  );
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => {
    process.stderr.write(String(e?.stack ?? e) + "\n");
    process.exit(1);
  });
}
