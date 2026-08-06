import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { CalculatorRepository } from "./calculator.ts";
import { openDesktopDatabase, type DesktopDatabase } from "./database.ts";
import { NotebookRepository } from "./notebook.ts";
import { PrinciplesRepository } from "./principles.ts";
import { TagsRepository } from "./tags.ts";
import { TradingRepository } from "./trading.ts";

const schema = readFileSync(new URL("./schema.sql", import.meta.url), "utf8");
const open = (): DesktopDatabase => openDesktopDatabase(":memory:", schema);

test("notebook mutations keep rows and outbox writes atomic", () => {
  const store = open();
  const notebook = new NotebookRepository(store);
  const documentJson = JSON.stringify({ root: { children: [{ type: "heading", tag: "h1", children: [{ text: "Plan" }] }] } });
  const id = notebook.createNote({
    accountId: "account",
    folderId: null,
    documentJson,
    seedUpdateB64: Buffer.from("seed").toString("base64"),
    seedStateVectorB64: Buffer.from("vector").toString("base64"),
  });
  assert.equal(notebook.notes("account")[0]?.title, "Plan");
  assert.deepEqual(notebook.noteUpdates(id), [Buffer.from("seed").toString("base64")]);
  assert.equal(store.db.prepare("SELECT count(*) AS count FROM outbox WHERE name = 'createNote'").get()?.count, 1);
  assert.equal(store.db.prepare("SELECT count(*) AS count FROM outbox WHERE name = 'appendNoteUpdate'").get()?.count, 0);
  const folder = notebook.createFolder("account", "Setups");
  notebook.moveNote(id, folder, 3);
  assert.equal(notebook.notes("account")[0]?.folderId, folder);
  store.close();
});

test("playbook and journal CRUD preserve derived metrics and mutation payloads", () => {
  const store = open();
  const trading = new TradingRepository(store);
  const playbook = trading.createPlaybook({ name: "Breakout", edgeName: "Momentum" });
  assert.equal(trading.updatePlaybook(playbook.id, { name: "Breakout v2" }).edgeName, "Momentum");
  const trade = trading.createJournalEntry({
    accountId: "account",
    openDate: "2026-01-01T00:00:00Z",
    closeDate: "2026-01-01T01:00:00Z",
    entryPrice: 100,
    exitPrice: 110,
    positionSize: 10,
    stopLoss: 95,
    symbol: "AAPL",
    tradeType: "long",
    playbookId: playbook.id,
  });
  assert.equal(trade.totalPl, 10);
  assert.equal(trade.riskReward, 2);
  assert.equal(trading.updateJournalEntry(trade.id, { exitPrice: 90 }).status, "loss");
  assert.deepEqual(
    store.db.prepare("SELECT name FROM outbox ORDER BY mutation_id").all().map((row) => row.name),
    ["createPlaybook", "updatePlaybook", "createJournalEntry", "updateJournalEntry"],
  );
  store.close();
});

test("tag merge deduplicates local trade tags without echoing a trade mutation", () => {
  const store = open();
  const tags = new TagsRepository(store);
  const category = tags.createCategory("Custom", null);
  const from = tags.createTag(category.id, "From", null);
  const into = tags.createTag(category.id, "Into", null);
  store.db
    .prepare(
      `INSERT INTO journal_entries
       (id, account_id, open_date, close_date, entry_price, exit_price, position_size, symbol, trade_type, tag_ids)
       VALUES ('trade', 'account', '2026-01-01', '2026-01-01', 1, 1, 1, 'AAPL', 'long', ?)`,
    )
    .run(JSON.stringify([from.id, into.id]));
  tags.mergeTags(from.id, into.id);
  assert.deepEqual(JSON.parse(String(store.db.prepare("SELECT tag_ids FROM journal_entries WHERE id = 'trade'").get()?.tag_ids)), [into.id]);
  assert.equal(store.db.prepare("SELECT count(*) AS count FROM outbox WHERE name = 'mergeTags'").get()?.count, 1);
  assert.equal(store.db.prepare("SELECT count(*) AS count FROM outbox WHERE name LIKE '%JournalEntry'").get()?.count, 0);
  store.close();
});

test("principle statistics and calculator entities are computed and queued locally", () => {
  const store = open();
  const principles = new PrinciplesRepository(store);
  const principle = principles.create({ accountId: "account", title: "No revenge trades" });
  store.db
    .prepare(
      `INSERT INTO journal_entries
       (id, account_id, open_date, close_date, entry_price, exit_price, position_size, symbol,
        trade_type, total_pl, violated_principle_ids)
       VALUES ('trade', 'account', '2026-01-01', '2026-01-01', 100, 110, 2, 'AAPL', 'long', 10, ?)`,
    )
    .run(JSON.stringify([principle.id]));
  assert.equal(principles.principles("account")[0]?.violatedCumulativeProfit, 20);

  const calculator = new CalculatorRepository(store);
  const first = calculator.upsertRule({ accountId: "account", accountBalance: 10_000, accountRisk: 1, maxStopLossPct: 2 });
  const second = calculator.upsertRule({ accountId: "account", accountBalance: 20_000, accountRisk: 2, maxStopLossPct: 3 });
  assert.equal(first.id, second.id);
  const plan = calculator.createPlan({
    symbol: "AAPL", positionType: "long", entryPrice: 100, stopLoss: 95,
    accountBalance: 10_000, accountRisk: 1, totalShares: 100, positionValue: 10_000,
    tranchesJson: "[]",
  });
  assert.equal(calculator.updatePlan(plan.id, { status: "completed" }).status, "completed");
  assert.equal(calculator.history().length, 0);
  store.close();
});
