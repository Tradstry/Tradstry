import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { openDesktopDatabase } from "./database.ts";
import { backendOrigin, MediaSync, verifyMediaBytes } from "./media.ts";

const schema = readFileSync(new URL("./schema.sql", import.meta.url), "utf8");

test("media routes are derived from the GraphQL backend origin", () => {
  assert.equal(backendOrigin("https://api.example/graphql"), "https://api.example");
  assert.equal(backendOrigin("https://api.example/"), "https://api.example");
});

test("media hash verification rejects corrupt bytes", () => {
  const bytes = Buffer.from("valid");
  const hash = createHash("sha256").update(bytes).digest("hex");
  assert.doesNotThrow(() => verifyMediaBytes(bytes, hash));
  assert.throws(() => verifyMediaBytes(Buffer.from("bad"), hash), /media hash mismatch/);
});

test("flush uploads pending media and marks only successful rows", async () => {
  const directory = mkdtempSync(join(tmpdir(), "tradstry-media-"));
  try {
    const path = join(directory, "hash");
    writeFileSync(path, "bytes");
    const store = openDesktopDatabase(":memory:", schema);
    store.db
      .prepare(
        `INSERT INTO notebook_media
         (hash, note_id, account_id, mime, media_type, bytes, original_filename, local_path, upload_state)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending')`,
      )
      .run("hash", "note", "account", "image/png", "image", 5, "image.png", path);
    const requests: string[] = [];
    const media = new MediaSync({
      db: store.db,
      backendUrl: "https://api.example/graphql",
      getAccessToken: async () => "token",
      fetch: async (input) => {
        requests.push(String(input));
        return new Response(null, { status: 204 });
      },
    });
    assert.equal(await media.flush("account"), 1);
    assert.deepEqual(requests, ["https://api.example/notebook/media/upload"]);
    assert.equal(store.db.prepare("SELECT upload_state FROM notebook_media WHERE hash = ?").get("hash")?.upload_state, "uploaded");
    store.close();
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
