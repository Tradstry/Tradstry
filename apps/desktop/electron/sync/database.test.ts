import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { openDesktopDatabase, transaction } from "./database.ts";

const schema = readFileSync(new URL("./schema.sql", import.meta.url), "utf8");

test("database initialization is idempotent and persists one client id", () => {
  const first = openDesktopDatabase(":memory:", schema);
  assert.ok(first.clientId);
  assert.equal(first.db.prepare("SELECT count(*) AS count FROM client").get()?.count, 1);
  first.close();
});

test("transaction rolls back failed operations", () => {
  const store = openDesktopDatabase(":memory:", schema);
  assert.throws(() =>
    transaction(store.db, () => {
      store.db.prepare("INSERT INTO client (id) VALUES (?)").run("second");
      throw new Error("stop");
    }),
  );
  assert.equal(store.db.prepare("SELECT count(*) AS count FROM client").get()?.count, 1);
  store.close();
});
