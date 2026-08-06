import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { openDesktopDatabase, type DesktopDatabase } from "./database.ts";
import { REPLAY_WINDOW, SyncEngine, type SyncTransport } from "./engine.ts";
import type {
  CalculatorPullResult,
  JournalPullResult,
  OutboxRow,
  PlaybookPullResult,
  PrinciplePullResult,
  PullResult,
  RemoteUpdate,
  TagsPullResult,
  WireAccount,
} from "./protocol.ts";

const schema = readFileSync(new URL("./schema.sql", import.meta.url), "utf8");

class FakeTransport implements SyncTransport {
  calls: string[] = [];
  acknowledgements: number[] = [];
  updates: RemoteUpdate[] = [];
  failPush = false;

  async push(_clientId: string, _accountId: string, mutations: OutboxRow[]): Promise<number> {
    this.calls.push(`push:${mutations.map((row) => row.id).join(",")}`);
    if (this.failPush) throw new Error("offline");
    return this.acknowledgements.shift() ?? mutations.at(-1)?.id ?? 0;
  }

  async pull(): Promise<PullResult> {
    this.calls.push("pull");
    return { cookie: "next", lastMutationId: 0, notes: [], folders: [] };
  }

  async pullUpdates(_accountId: string, sinceSeq: number): Promise<RemoteUpdate[]> {
    this.calls.push(`updates:${sinceSeq}`);
    return this.updates;
  }

  async pullPlaybook(): Promise<PlaybookPullResult> {
    return { cookie: null, lastMutationId: 0, playbooks: [] };
  }

  async pullJournal(): Promise<JournalPullResult> {
    return { cookie: null, lastMutationId: 0, entries: [] };
  }

  async pullPrinciple(): Promise<PrinciplePullResult> {
    return { cookie: null, lastMutationId: 0, principles: [] };
  }

  async pullTags(): Promise<TagsPullResult> {
    return { cookie: null, lastMutationId: 0, categories: [], tags: [] };
  }

  async pullCalculator(): Promise<CalculatorPullResult> {
    return { cookie: null, lastMutationId: 0, rules: [], plans: [], history: [] };
  }

  async pullAccounts(): Promise<WireAccount[]> {
    return [];
  }
}

function store(): DesktopDatabase {
  return openDesktopDatabase(":memory:", schema);
}

function enqueue(database: DesktopDatabase, count: number): void {
  for (let id = 1; id <= count; id += 1) {
    database.db
      .prepare("INSERT INTO outbox (name, args, hlc) VALUES (?, ?, ?)")
      .run(`mutation-${id}`, "{}", String(id));
  }
}

test("outbox is fully drained before pull, including partial acknowledgements", async () => {
  const database = store();
  enqueue(database, 2);
  const transport = new FakeTransport();
  transport.acknowledgements = [1, 2];
  const report = await new SyncEngine(database, transport).syncAccount("account");
  assert.deepEqual(transport.calls.slice(0, 3), ["push:1,2", "push:2", "pull"]);
  assert.equal(report.pushed, 2);
  assert.equal(database.db.prepare("SELECT count(*) AS count FROM outbox").get()?.count, 0);
  database.close();
});

test("a failed push prevents pull from rebasing unpushed work", async () => {
  const database = store();
  enqueue(database, 1);
  const transport = new FakeTransport();
  transport.failPush = true;
  await assert.rejects(() => new SyncEngine(database, transport).syncAccount("account"), /offline/);
  assert.deepEqual(transport.calls, ["push:1"]);
  assert.equal(database.db.prepare("SELECT count(*) AS count FROM outbox").get()?.count, 1);
  database.close();
});

test("remote updates use a replay window and remain idempotent", async () => {
  const database = store();
  database.db.prepare("INSERT INTO update_cursor (account_id, last_seq) VALUES (?, ?)").run("account", 100);
  const transport = new FakeTransport();
  transport.updates = [{ noteId: "note", seq: 101, update: Buffer.from("update").toString("base64") }];
  const engine = new SyncEngine(database, transport);
  const first = await engine.syncAccount("account");
  const second = await engine.syncAccount("account");
  assert.ok(transport.calls.includes(`updates:${100 - REPLAY_WINDOW}`));
  assert.equal(first.pulledUpdates, 1);
  assert.equal(second.pulledUpdates, 0);
  assert.equal(database.db.prepare("SELECT count(*) AS count FROM note_updates").get()?.count, 1);
  database.close();
});

test("server notebook rows update cursors and preserve newer local metadata", async () => {
  const database = store();
  database.db
    .prepare(
      `INSERT INTO notes
       (id, account_id, folder_id, title, document_json, sort_order, trade_ids,
        hlc_folder_id, hlc_sort_order, hlc_trade_ids, body_hlc, sync_state)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run("note", "account", "local-folder", "Local", "{}", 1, "[]", "999", "001", "001", "999", "pending");
  const transport = new FakeTransport();
  transport.pull = async () => ({
    cookie: "opaque-cookie",
    lastMutationId: 0,
    folders: [],
    notes: [{
      id: "note",
      folderId: "server-folder",
      title: "Server",
      documentJson: "{\"server\":true}",
      sortOrder: 2,
      tradeIds: [],
      hlc: "100",
      deletedAt: null,
      updatedAt: "now",
    }],
  });
  await new SyncEngine(database, transport).syncAccount("account");
  const note = database.db.prepare("SELECT folder_id, sort_order, document_json FROM notes WHERE id = ?").get("note") as Record<string, unknown>;
  assert.equal(note.folder_id, "local-folder");
  assert.equal(note.sort_order, 2);
  assert.equal(note.document_json, "{}");
  assert.equal(database.db.prepare("SELECT cookie FROM sync_meta WHERE account_id = ?").get("account")?.cookie, "opaque-cookie");
  database.close();
});

test("all secondary sync channels apply rows and advance independent cursors", async () => {
  const database = store();
  database.db.prepare("INSERT INTO sync_meta (account_id) VALUES ('account')").run();
  const transport = new FakeTransport();
  transport.pullPlaybook = async () => ({
    cookie: "playbook-cookie", lastMutationId: 0,
    playbooks: [{ id: "playbook", name: "Breakout", edgeName: "Momentum", entryRules: "e", exitRules: "x", positionSizingRules: "p", additionalRules: null, hlc: "100", deletedAt: null, updatedAt: "now" }],
  });
  transport.pullJournal = async () => ({
    cookie: "journal-cookie", lastMutationId: 0,
    entries: [{ id: "trade", openDate: "2026-01-01", closeDate: "2026-01-02", entryPrice: 100, exitPrice: 110, positionSize: 1, stopLoss: 95, symbol: "AAPL", symbolName: "Apple", status: "profit", totalPl: 10, netRoi: 10, duration: 86400, riskReward: 2, tradeType: "long", playbookId: "playbook", notes: null, broke30MinRule: false, preTradeConviction: 5, marketRegime: "trend", isPlannedPreMarket: true, revengeTrade: false, ruleAdherenceScore: 5, tagIds: ["tag"], hlc: "101", deletedAt: null, updatedAt: "now" }],
  });
  transport.pullPrinciple = async () => ({
    cookie: "principle-cookie", lastMutationId: 0,
    principles: [{ id: "principle", accountId: "account", playbookId: null, evidenceNoteId: null, title: "Rule", theRule: "Wait", why: "Discipline", intervention: null, priority: 1, isActive: true, hlc: "102", deletedAt: null, updatedAt: "now" }],
  });
  transport.pullTags = async () => ({
    cookie: "tag-cookie", lastMutationId: 0,
    categories: [{ id: "category", name: "Mistakes", role: "mistakes", color: null, sortOrder: 0, hlc: "103", deletedAt: null, updatedAt: "now" }],
    tags: [{ id: "tag", categoryId: "category", name: "FOMO", color: null, hlc: "104", deletedAt: null, updatedAt: "now" }],
  });
  transport.pullCalculator = async () => ({
    cookie: "calculator-cookie", lastMutationId: 0,
    rules: [{ id: "rule", accountId: "account", accountBalance: 10000, accountRisk: 1, maxStopLossPct: 2, hlc: "105", deletedAt: null, updatedAt: "now" }],
    plans: [], history: [],
  });
  transport.pullAccounts = async () => [{ id: "account", name: "Main", broker: null, currency: "USD", icon: null, totalValue: 10000, riskProfile: null }];
  await new SyncEngine(database, transport).syncAll();
  assert.equal(database.db.prepare("SELECT name FROM playbooks WHERE id = 'playbook'").get()?.name, "Breakout");
  assert.equal(database.db.prepare("SELECT symbol FROM journal_entries WHERE id = 'trade'").get()?.symbol, "AAPL");
  assert.equal(database.db.prepare("SELECT title FROM trading_principles WHERE id = 'principle'").get()?.title, "Rule");
  assert.equal(database.db.prepare("SELECT name FROM tags_cache WHERE id = 'tag'").get()?.name, "FOMO");
  assert.equal(database.db.prepare("SELECT account_balance FROM calc_rules WHERE account_id = 'account'").get()?.account_balance, 10000);
  assert.equal(database.db.prepare("SELECT name FROM accounts_cache WHERE id = 'account'").get()?.name, "Main");
  assert.equal(database.db.prepare("SELECT cookie FROM tag_sync WHERE id = 1").get()?.cookie, "tag-cookie");
  database.close();
});
