import type { DatabaseSync, SQLInputValue } from "node:sqlite";
import type { DesktopDatabase } from "./database.ts";
import { transaction } from "./database.ts";
import { mergeNote, type NoteRow } from "./merge.ts";
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
  WireFolder,
  WireJournalEntry,
  WireNote,
  WirePrinciple,
} from "./protocol.ts";

export const NOTE_UPDATES_EVENT = "notebook://updates";
export const PUSH_BATCH_MAX = 100;
export const REPLAY_WINDOW = 64;

export type SyncReport = {
  pushed: number;
  pulledNotes: number;
  pulledFolders: number;
  pulledUpdates: number;
  updatedNotes: string[];
};

export interface SyncTransport {
  push(clientId: string, accountId: string, mutations: OutboxRow[]): Promise<number>;
  pull(clientId: string, accountId: string, cookie: string | null): Promise<PullResult>;
  pullUpdates(accountId: string, sinceSeq: number): Promise<RemoteUpdate[]>;
  pullPlaybook(clientId: string, cookie: string | null): Promise<PlaybookPullResult>;
  pullJournal(clientId: string, accountId: string, cookie: string | null): Promise<JournalPullResult>;
  pullPrinciple(clientId: string, accountId: string, cookie: string | null): Promise<PrinciplePullResult>;
  pullTags(clientId: string, cookie: string | null): Promise<TagsPullResult>;
  pullCalculator(clientId: string, cookie: string | null): Promise<CalculatorPullResult>;
  pullAccounts(): Promise<WireAccount[]>;
}

export interface MediaFlusher {
  flush(accountId: string): Promise<number>;
}

export type SyncLogger = Pick<Console, "error">;

type StoredNote = {
  id: string;
  folder_id: string | null;
  title: string;
  document_json: string;
  sort_order: number;
  trade_ids: string;
  hlc_folder_id: string;
  hlc_sort_order: string;
  hlc_trade_ids: string;
  body_hlc: string;
  deleted_at: string | null;
};

type WholeRow = Record<string, SQLInputValue>;

function emptyReport(): SyncReport {
  return { pushed: 0, pulledNotes: 0, pulledFolders: 0, pulledUpdates: 0, updatedNotes: [] };
}

function nullableString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function cursor(db: DatabaseSync, table: string, where: string, values: SQLInputValue[]): string | null {
  const row = db.prepare(`SELECT cookie FROM ${table} WHERE ${where}`).get(...values) as
    | { cookie: string | null }
    | undefined;
  return row?.cookie ?? null;
}

function observeAll(store: DesktopDatabase, stamps: string[]): void {
  for (const stamp of stamps) store.hlc.observe(stamp);
}

function upsertWholeRow(
  db: DatabaseSync,
  table: string,
  keyColumn: string,
  key: SQLInputValue,
  fields: WholeRow,
  hlc: string,
  deletedAt: string | null,
): void {
  const local = db.prepare(`SELECT hlc FROM ${table} WHERE ${keyColumn} = ?`).get(key) as
    | { hlc: string }
    | undefined;
  const columns = Object.keys(fields);

  if (!local) {
    const names = [keyColumn, ...columns, "hlc", "deleted_at", "sync_state"];
    const placeholders = names.map(() => "?").join(", ");
    db.prepare(`INSERT INTO ${table} (${names.join(", ")}) VALUES (${placeholders})`).run(
      key,
      ...Object.values(fields),
      hlc,
      deletedAt,
      "synced",
    );
    return;
  }

  if (hlc > local.hlc) {
    const assignments = [...columns.map((name) => `${name} = ?`), "hlc = ?"].join(", ");
    db.prepare(`UPDATE ${table} SET ${assignments} WHERE ${keyColumn} = ?`).run(
      ...Object.values(fields),
      hlc,
      key,
    );
  }
  db.prepare(
    `UPDATE ${table} SET deleted_at = COALESCE(deleted_at, ?), sync_state = 'synced' WHERE ${keyColumn} = ?`,
  ).run(deletedAt, key);
}

export class SyncEngine {
  readonly #store: DesktopDatabase;
  readonly #transport: SyncTransport;
  readonly #media: MediaFlusher | undefined;
  readonly #logger: SyncLogger;

  constructor(
    store: DesktopDatabase,
    transport: SyncTransport,
    options: { media?: MediaFlusher; logger?: SyncLogger } = {},
  ) {
    this.#store = store;
    this.#transport = transport;
    this.#media = options.media;
    this.#logger = options.logger ?? console;
  }

  async syncAccount(accountId: string): Promise<SyncReport> {
    const report = emptyReport();
    await this.#flushOutbox(accountId, report);
    if (this.#media) {
      try {
        await this.#media.flush(accountId);
      } catch (error) {
        this.#logger.error(`media sync (${accountId}):`, error);
      }
    }
    await this.#pullNotebook(accountId, report);
    await this.#pullUpdates(accountId, report);
    try {
      await this.#pullJournal(accountId);
    } catch (error) {
      this.#logger.error(`journal sync (${accountId}):`, error);
    }
    try {
      await this.#pullPrinciples(accountId);
    } catch (error) {
      this.#logger.error(`principle sync (${accountId}):`, error);
    }
    return report;
  }

  async syncAll(): Promise<SyncReport[]> {
    const reports: SyncReport[] = [];
    for (const accountId of this.discoverAccounts()) {
      try {
        reports.push(await this.syncAccount(accountId));
      } catch (error) {
        this.#logger.error(`notebook sync (${accountId}):`, error);
      }
    }
    for (const [name, operation] of [
      ["playbook", () => this.#pullPlaybooks()],
      ["tag", () => this.#pullTags()],
      ["calculator", () => this.#pullCalculator()],
      ["account", () => this.#refreshAccounts()],
    ] as const) {
      try {
        await operation();
      } catch (error) {
        this.#logger.error(`${name} sync:`, error);
      }
    }
    return reports;
  }

  discoverAccounts(): string[] {
    const rows = this.#store.db
      .prepare(
        `SELECT account_id FROM notes
         UNION SELECT account_id FROM folders
         UNION SELECT account_id FROM sync_meta`,
      )
      .all() as Array<{ account_id: string }>;
    return rows.map((row) => row.account_id);
  }

  async #flushOutbox(accountId: string, report: SyncReport): Promise<void> {
    for (;;) {
      const batch = this.#store.db
        .prepare(
          "SELECT mutation_id AS id, name, args, hlc FROM outbox ORDER BY mutation_id ASC LIMIT ?",
        )
        .all(PUSH_BATCH_MAX) as OutboxRow[];
      if (batch.length === 0) return;

      const last = await this.#transport.push(this.#store.clientId, accountId, batch);
      const removed = this.#store.db.prepare("DELETE FROM outbox WHERE mutation_id <= ?").run(last).changes;
      this.#store.db
        .prepare(
          `INSERT INTO sync_meta (account_id, last_mutation_id) VALUES (?, ?)
           ON CONFLICT(account_id) DO UPDATE SET
             last_mutation_id = MAX(last_mutation_id, excluded.last_mutation_id)`,
        )
        .run(accountId, last);
      report.pushed += Number(removed);
      if (removed === 0) throw new Error(`push acked ${last} but drained no outbox rows; aborting to avoid a spin`);
    }
  }

  async #pullNotebook(accountId: string, report: SyncReport): Promise<void> {
    const cookieValue = cursor(this.#store.db, "sync_meta", "account_id = ?", [accountId]);
    const result = await this.#transport.pull(this.#store.clientId, accountId, cookieValue);
    for (const folder of result.folders) {
      this.#applyFolder(accountId, folder);
      report.pulledFolders += 1;
    }
    for (const note of result.notes) {
      this.#applyNote(accountId, note);
      report.pulledNotes += 1;
    }
    this.#store.db
      .prepare(
        `INSERT INTO sync_meta (account_id, cookie, last_sync_at) VALUES (?, ?, datetime('now'))
         ON CONFLICT(account_id) DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at`,
      )
      .run(accountId, result.cookie);
  }

  async #pullUpdates(accountId: string, report: SyncReport): Promise<void> {
    const cursorRow = this.#store.db
      .prepare("SELECT last_seq FROM update_cursor WHERE account_id = ?")
      .get(accountId) as { last_seq: number } | undefined;
    const since = Math.max(0, (cursorRow?.last_seq ?? 0) - REPLAY_WINDOW);
    const rows = await this.#transport.pullUpdates(accountId, since);
    if (rows.length === 0) return;

    transaction(this.#store.db, () => {
      let highest = 0;
      for (const row of rows) {
        highest = Math.max(highest, row.seq);
        const fresh = this.#store.db.prepare("INSERT OR IGNORE INTO pulled_seq (seq) VALUES (?)").run(row.seq).changes;
        if (fresh === 0) continue;
        const bytes = decodeBase64(row.update);
        this.#store.db
          .prepare('INSERT INTO note_updates (note_id, "update", synced) VALUES (?, ?, 1)')
          .run(row.noteId, bytes);
        report.pulledUpdates += 1;
        if (!report.updatedNotes.includes(row.noteId)) report.updatedNotes.push(row.noteId);
      }
      this.#store.db
        .prepare(
          `INSERT INTO update_cursor (account_id, last_seq) VALUES (?, ?)
           ON CONFLICT(account_id) DO UPDATE SET last_seq = MAX(last_seq, excluded.last_seq)`,
        )
        .run(accountId, highest);
    });
  }

  #applyFolder(accountId: string, row: WireFolder): void {
    this.#store.hlc.observe(row.hlc);
    const local = this.#store.db
      .prepare("SELECT hlc_name, hlc_parent, hlc_sort_order, deleted_at FROM folders WHERE id = ?")
      .get(row.id) as
      | { hlc_name: string; hlc_parent: string; hlc_sort_order: string; deleted_at: string | null }
      | undefined;
    if (!local) {
      this.#store.db
        .prepare(
          `INSERT INTO folders
           (id, account_id, parent_folder_id, name, sort_order, is_system, hlc_name, hlc_parent, hlc_sort_order, deleted_at, sync_state)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'synced')`,
        )
        .run(row.id, accountId, row.parentFolderId, row.name, row.sortOrder, Number(row.isSystem), row.hlc, row.hlc, row.hlc, row.deletedAt);
      return;
    }
    this.#store.db
      .prepare(
        `UPDATE folders SET
          name = CASE WHEN ? > hlc_name THEN ? ELSE name END,
          hlc_name = CASE WHEN ? > hlc_name THEN ? ELSE hlc_name END,
          parent_folder_id = CASE WHEN ? > hlc_parent THEN ? ELSE parent_folder_id END,
          hlc_parent = CASE WHEN ? > hlc_parent THEN ? ELSE hlc_parent END,
          sort_order = CASE WHEN ? > hlc_sort_order THEN ? ELSE sort_order END,
          hlc_sort_order = CASE WHEN ? > hlc_sort_order THEN ? ELSE hlc_sort_order END,
          deleted_at = COALESCE(deleted_at, ?), is_system = ?, sync_state = 'synced'
         WHERE id = ?`,
      )
      .run(row.hlc, row.name, row.hlc, row.hlc, row.hlc, row.parentFolderId, row.hlc, row.hlc, row.hlc, row.sortOrder, row.hlc, row.hlc, row.deletedAt, Number(row.isSystem), row.id);
  }

  #applyNote(accountId: string, row: WireNote): void {
    this.#store.hlc.observe(row.hlc);
    const server: NoteRow = {
      id: row.id,
      folderId: row.folderId,
      title: row.title,
      documentJson: row.documentJson,
      sortOrder: row.sortOrder,
      tradeIds: row.tradeIds,
      hlcFolderId: row.hlc,
      hlcSortOrder: row.hlc,
      hlcTradeIds: row.hlc,
      bodyHlc: row.hlc,
      deletedAt: row.deletedAt,
    };
    transaction(this.#store.db, () => {
      const stored = this.#store.db
        .prepare(
          `SELECT id, folder_id, title, document_json, sort_order, trade_ids,
                  hlc_folder_id, hlc_sort_order, hlc_trade_ids, body_hlc, deleted_at
           FROM notes WHERE id = ?`,
        )
        .get(row.id) as StoredNote | undefined;
      if (!stored) return this.#writeNote(accountId, server);
      const local = storedToNote(stored);
      const merged = mergeNote(local, server);
      if (merged.kind === "take") this.#writeNote(accountId, merged.note);
      if (merged.kind === "tombstone") {
        this.#store.db
          .prepare(
            "UPDATE notes SET deleted_at = COALESCE(deleted_at, ?, datetime('now')), sync_state = 'synced' WHERE id = ?",
          )
          .run(row.deletedAt, row.id);
      }
    });
  }

  #writeNote(accountId: string, row: NoteRow): void {
    this.#store.db
      .prepare(
        `INSERT INTO notes
         (id, account_id, folder_id, title, document_json, sort_order, trade_ids,
          hlc_folder_id, hlc_sort_order, hlc_trade_ids, body_hlc, deleted_at, sync_state)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'synced')
         ON CONFLICT(id) DO UPDATE SET
           folder_id = excluded.folder_id, title = excluded.title,
           document_json = excluded.document_json, sort_order = excluded.sort_order,
           trade_ids = excluded.trade_ids, hlc_folder_id = excluded.hlc_folder_id,
           hlc_sort_order = excluded.hlc_sort_order, hlc_trade_ids = excluded.hlc_trade_ids,
           body_hlc = excluded.body_hlc, deleted_at = excluded.deleted_at, sync_state = 'synced'`,
      )
      .run(row.id, accountId, row.folderId, row.title, row.documentJson, row.sortOrder, JSON.stringify(row.tradeIds), row.hlcFolderId, row.hlcSortOrder, row.hlcTradeIds, row.bodyHlc, row.deletedAt);
  }

  async #pullPlaybooks(): Promise<void> {
    const result = await this.#transport.pullPlaybook(
      this.#store.clientId,
      cursor(this.#store.db, "playbook_sync", "id = 1", []),
    );
    observeAll(this.#store, result.playbooks.map((row) => row.hlc));
    for (const row of result.playbooks) {
      upsertWholeRow(this.#store.db, "playbooks", "id", row.id, {
        name: row.name,
        edge_name: row.edgeName,
        entry_rules: row.entryRules,
        exit_rules: row.exitRules,
        position_sizing_rules: row.positionSizingRules,
        additional_rules: row.additionalRules,
      }, row.hlc, row.deletedAt);
    }
    this.#setSingletonCursor("playbook_sync", result.cookie);
  }

  async #pullJournal(accountId: string): Promise<void> {
    const result = await this.#transport.pullJournal(
      this.#store.clientId,
      accountId,
      cursor(this.#store.db, "journal_sync", "account_id = ?", [accountId]),
    );
    for (const row of result.entries) this.#applyJournal(accountId, row);
    this.#store.db
      .prepare(
        `INSERT INTO journal_sync (account_id, cookie, last_sync_at) VALUES (?, ?, datetime('now'))
         ON CONFLICT(account_id) DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at`,
      )
      .run(accountId, result.cookie);
  }

  #applyJournal(accountId: string, row: WireJournalEntry): void {
    this.#store.hlc.observe(row.hlc);
    upsertWholeRow(this.#store.db, "journal_entries", "id", row.id, {
      account_id: accountId,
      open_date: row.openDate,
      close_date: row.closeDate,
      entry_price: row.entryPrice,
      exit_price: row.exitPrice,
      position_size: row.positionSize,
      stop_loss: row.stopLoss,
      symbol: row.symbol,
      symbol_name: row.symbolName,
      status: row.status,
      total_pl: row.totalPl,
      net_roi: row.netRoi,
      duration: row.duration,
      risk_reward: row.riskReward,
      trade_type: row.tradeType,
      playbook_id: row.playbookId,
      notes: row.notes,
      broke_30min_rule: booleanValue(row.broke30MinRule),
      pre_trade_conviction: row.preTradeConviction,
      market_regime: row.marketRegime,
      is_planned_pre_market: booleanValue(row.isPlannedPreMarket),
      revenge_trade: booleanValue(row.revengeTrade),
      rule_adherence_score: row.ruleAdherenceScore,
      tag_ids: JSON.stringify(row.tagIds),
    }, row.hlc, row.deletedAt);
  }

  async #pullPrinciples(accountId: string): Promise<void> {
    const result = await this.#transport.pullPrinciple(
      this.#store.clientId,
      accountId,
      cursor(this.#store.db, "principle_sync", "account_id = ?", [accountId]),
    );
    for (const row of result.principles) this.#applyPrinciple(accountId, row);
    this.#store.db
      .prepare(
        `INSERT INTO principle_sync (account_id, cookie, last_sync_at) VALUES (?, ?, datetime('now'))
         ON CONFLICT(account_id) DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at`,
      )
      .run(accountId, result.cookie);
  }

  #applyPrinciple(accountId: string, row: WirePrinciple): void {
    this.#store.hlc.observe(row.hlc);
    upsertWholeRow(this.#store.db, "trading_principles", "id", row.id, {
      account_id: accountId,
      playbook_id: row.playbookId,
      evidence_note_id: row.evidenceNoteId,
      title: row.title,
      the_rule: row.theRule,
      why: row.why,
      intervention: row.intervention,
      priority: row.priority,
      is_active: Number(row.isActive),
    }, row.hlc, row.deletedAt);
  }

  async #pullTags(): Promise<void> {
    const result = await this.#transport.pullTags(
      this.#store.clientId,
      cursor(this.#store.db, "tag_sync", "id = 1", []),
    );
    for (const row of result.categories) {
      this.#store.hlc.observe(row.hlc);
      upsertWholeRow(this.#store.db, "tag_categories_cache", "id", row.id, {
        name: row.name,
        role: row.role,
        color: row.color,
        sort_order: row.sortOrder,
      }, row.hlc, row.deletedAt);
    }
    for (const row of result.tags) {
      this.#store.hlc.observe(row.hlc);
      upsertWholeRow(this.#store.db, "tags_cache", "id", row.id, {
        name: row.name,
        color: row.color,
        category_id: row.categoryId,
      }, row.hlc, row.deletedAt);
    }
    this.#setSingletonCursor("tag_sync", result.cookie);
  }

  async #pullCalculator(): Promise<void> {
    const result = await this.#transport.pullCalculator(
      this.#store.clientId,
      cursor(this.#store.db, "calculator_sync", "id = 1", []),
    );
    for (const row of result.rules) {
      this.#store.hlc.observe(row.hlc);
      upsertWholeRow(this.#store.db, "calc_rules", "account_id", row.accountId, {
        id: row.id,
        account_balance: row.accountBalance,
        account_risk: row.accountRisk,
        max_stop_loss_pct: row.maxStopLossPct,
      }, row.hlc, row.deletedAt);
    }
    for (const row of result.plans) {
      this.#store.hlc.observe(row.hlc);
      upsertWholeRow(this.#store.db, "calc_plans", "id", row.id, {
        symbol: row.symbol,
        position_type: row.positionType,
        entry_price: row.entryPrice,
        stop_loss: row.stopLoss,
        account_balance: row.accountBalance,
        account_risk: row.accountRisk,
        total_shares: row.totalShares,
        position_value: row.positionValue,
        status: row.status,
        tranches_json: row.tranchesJson,
        notes: row.notes,
      }, row.hlc, row.deletedAt);
    }
    for (const row of result.history) {
      this.#store.hlc.observe(row.hlc);
      upsertWholeRow(this.#store.db, "calc_history", "id", row.id, {
        symbol: row.symbol,
        position_type: row.positionType,
        entry_price: row.entryPrice,
        stop_loss: row.stopLoss,
        account_balance: row.accountBalance,
        account_risk: row.accountRisk,
        shares: row.shares,
        position_value: row.positionValue,
        account_pct: row.accountPct,
        stop_loss_pct: row.stopLossPct,
      }, row.hlc, row.deletedAt);
    }
    this.#setSingletonCursor("calculator_sync", result.cookie);
  }

  async #refreshAccounts(): Promise<void> {
    const accounts = await this.#transport.pullAccounts();
    for (const row of accounts) {
      this.#store.db
        .prepare(
          `INSERT INTO accounts_cache (id, name, broker, currency, icon, total_value, risk_profile)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET name = excluded.name, broker = excluded.broker,
             currency = excluded.currency, icon = excluded.icon, total_value = excluded.total_value,
             risk_profile = excluded.risk_profile`,
        )
        .run(row.id, row.name, row.broker, row.currency, row.icon, row.totalValue, row.riskProfile);
    }
  }

  #setSingletonCursor(table: "playbook_sync" | "tag_sync" | "calculator_sync", cookieValue: string | null): void {
    this.#store.db
      .prepare(
        `INSERT INTO ${table} (id, cookie, last_sync_at) VALUES (1, ?, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at`,
      )
      .run(cookieValue);
  }
}

function booleanValue(value: boolean | null): number | null {
  return value === null ? null : Number(value);
}

function storedToNote(row: StoredNote): NoteRow {
  let tradeIds: string[] = [];
  try {
    const value: unknown = JSON.parse(row.trade_ids);
    if (Array.isArray(value) && value.every((item) => typeof item === "string")) tradeIds = value;
  } catch {
    // Match the old store: corrupt cached trade ids fall back to an empty set.
  }
  return {
    id: row.id,
    folderId: row.folder_id,
    title: row.title,
    documentJson: row.document_json,
    sortOrder: row.sort_order,
    tradeIds,
    hlcFolderId: row.hlc_folder_id,
    hlcSortOrder: row.hlc_sort_order,
    hlcTradeIds: row.hlc_trade_ids,
    bodyHlc: row.body_hlc,
    deletedAt: nullableString(row.deleted_at),
  };
}

function decodeBase64(value: string): Uint8Array {
  const compact = value.trim();
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(compact)) {
    throw new Error("invalid base64 update");
  }
  return Buffer.from(compact, "base64");
}
