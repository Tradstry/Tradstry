import type { GraphqlClient } from "./protocol.ts";
import type { DesktopDatabase } from "./database.ts";
import { CalculatorRepository } from "./calculator.ts";
import type { SyncEngine } from "./engine.ts";
import { MediaRepository } from "./media.ts";
import { NotebookRepository } from "./notebook.ts";
import { PrinciplesRepository } from "./principles.ts";
import { TagsRepository } from "./tags.ts";
import { TradingRepository } from "./trading.ts";

const MARKET_QUOTES = `query DesktopMarketQuotes($symbols: [String!]!) {
  marketQuotes(symbols: $symbols) {
    fetchedAt
    errors { symbol message }
    quotes {
      symbol name price change changePercent regularMarketPrice preMarketPrice postMarketPrice
      currency currencySymbol exchange marketState marketTime isStale
    }
  }
}`;

export type AuthCommands = {
  signIn(): Promise<unknown>;
  status(): Promise<unknown>;
  signOut(): Promise<void>;
  accessToken?(): Promise<string | null>;
};

export type AnalyticsCommands = {
  journal(accountId: string, timeFilter: unknown): unknown;
  calendar(accountId: string, year: number, month: number): unknown;
  advanced(accountId: string, timeFilter: unknown): unknown;
};

export class DesktopService {
  readonly #store: DesktopDatabase;
  readonly #auth: AuthCommands;
  readonly #sync: SyncEngine;
  readonly #graphql: GraphqlClient;
  readonly #analytics: AnalyticsCommands;
  readonly #notebook: NotebookRepository;
  readonly #trading: TradingRepository;
  readonly #tags: TagsRepository;
  readonly #principles: PrinciplesRepository;
  readonly #calculator: CalculatorRepository;
  readonly #media: MediaRepository;

  constructor(options: {
    store: DesktopDatabase;
    auth: AuthCommands;
    sync: SyncEngine;
    graphql: GraphqlClient;
    analytics: AnalyticsCommands;
    media: MediaRepository;
  }) {
    this.#store = options.store;
    this.#auth = options.auth;
    this.#sync = options.sync;
    this.#graphql = options.graphql;
    this.#analytics = options.analytics;
    this.#media = options.media;
    this.#notebook = new NotebookRepository(options.store);
    this.#trading = new TradingRepository(options.store);
    this.#tags = new TagsRepository(options.store);
    this.#principles = new PrinciplesRepository(options.store);
    this.#calculator = new CalculatorRepository(options.store);
  }

  async invoke(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
    switch (command) {
      case "sign_in": return this.#auth.signIn();
      case "auth_status": return this.#auth.status();
      case "auth_token": {
        if (!this.#auth.accessToken) throw new Error("Access tokens are unavailable");
        return this.#auth.accessToken();
      }
      case "sign_out": return this.#auth.signOut();
      case "graphql_query": return this.#graphql(
        stringArg(args, "query"),
        optionalObjectArg(args, "variables"),
      );
      case "accounts": return this.#accounts();
      case "market_watchlist_symbols": return this.#marketWatchlistSymbols();
      case "market_quotes": return this.#marketQuotes(stringArrayArg(args, "symbols"));
      case "journal_analytics": return this.#analytics.journal(stringArg(args, "accountId"), args.timeFilter);
      case "calendar_analytics": return this.#analytics.calendar(stringArg(args, "accountId"), numberArg(args, "year"), numberArg(args, "month"));
      case "advanced_analytics": return this.#analytics.advanced(stringArg(args, "accountId"), args.timeFilter);
      case "playbooks": return this.#trading.playbooks();
      case "create_playbook": return this.#trading.createPlaybook(objectArg(args, "input") as never);
      case "update_playbook": return this.#trading.updatePlaybook(stringArg(args, "id"), objectArg(args, "input") as never);
      case "delete_playbook": return this.#trading.deletePlaybook(stringArg(args, "id"));
      case "playbook_stats": return this.#playbookStats();
      case "journal_entries": return this.#trading.journalEntries(stringArg(args, "accountId"));
      case "create_journal_entry": return this.#trading.createJournalEntry(objectArg(args, "input") as never);
      case "update_journal_entry": return this.#trading.updateJournalEntry(stringArg(args, "id"), objectArg(args, "input") as never);
      case "delete_journal_entry": return this.#trading.deleteJournalEntry(stringArg(args, "id"));
      case "tags": return this.#tags.tags();
      case "tag_categories": return this.#tags.categories();
      case "create_tag": return this.#tags.createTag(stringArg(args, "categoryId"), stringArg(args, "name"), optionalString(args.color));
      case "create_tag_category": return this.#tags.createCategory(stringArg(args, "name"), optionalString(args.color));
      case "rename_tag_category": return this.#tags.renameCategory(stringArg(args, "id"), stringArg(args, "name"));
      case "set_tag_category_color": return this.#tags.setCategoryColor(stringArg(args, "id"), optionalString(args.color));
      case "reorder_tag_categories": return this.#tags.reorderCategories(arrayArg(args, "order") as never);
      case "delete_tag_category": return this.#tags.deleteCategory(stringArg(args, "id"));
      case "rename_tag": return this.#tags.renameTag(stringArg(args, "id"), stringArg(args, "name"));
      case "set_tag_color": return this.#tags.setTagColor(stringArg(args, "id"), optionalString(args.color));
      case "delete_tag": return this.#tags.deleteTag(stringArg(args, "id"));
      case "merge_tags": return this.#tags.mergeTags(stringArg(args, "fromId"), stringArg(args, "intoId"));
      case "principles": return this.#principles.principles(stringArg(args, "accountId"));
      case "create_principle": return this.#principles.create(objectArg(args, "input") as never);
      case "update_principle": return this.#principles.update(stringArg(args, "id"), objectArg(args, "input") as never);
      case "delete_principle": return this.#principles.delete(stringArg(args, "id"));
      case "reorder_principles": return this.#principles.reorder(stringArrayArg(args, "orderedIds"));
      case "position_calculator_rule": return this.#calculator.rule(stringArg(args, "accountId"));
      case "upsert_position_calculator_rule": return this.#calculator.upsertRule(objectArg(args, "input") as never);
      case "position_calculator_plans": return this.#calculator.plans();
      case "create_position_calculator_plan": return this.#calculator.createPlan(objectArg(args, "input") as never);
      case "update_position_calculator_plan": return this.#calculator.updatePlan(stringArg(args, "id"), objectArg(args, "input") as never);
      case "delete_position_calculator_plan": return this.#calculator.deletePlan(stringArg(args, "id"));
      case "position_calculator_history": return this.#calculator.history();
      case "create_position_calculator_history": return this.#calculator.createHistory(objectArg(args, "input") as never);
      case "delete_position_calculator_history": return this.#calculator.deleteHistory(stringArg(args, "id"));
      case "notebook_notes": return this.#notebook.notes(stringArg(args, "accountId"));
      case "notebook_folders": return this.#notebook.folders(stringArg(args, "accountId"));
      case "create_note": return this.#notebook.createNote({
        accountId: stringArg(args, "accountId"),
        folderId: optionalString(args.folderId),
        documentJson: stringArg(args, "documentJson"),
        seedUpdateB64: stringArg(args, "seedUpdateB64"),
        seedStateVectorB64: stringArg(args, "seedStateVectorB64"),
      });
      case "cache_note_body": return this.#notebook.cacheNoteBody(stringArg(args, "id"), stringArg(args, "documentJson"));
      case "note_updates": return this.#notebook.noteUpdates(stringArg(args, "noteId"));
      case "append_note_update": return this.#notebook.appendNoteUpdate(stringArg(args, "noteId"), stringArg(args, "updateB64"));
      case "move_note": return this.#notebook.moveNote(stringArg(args, "id"), optionalString(args.folderId), numberArg(args, "sortOrder"));
      case "delete_note": return this.#notebook.deleteNote(stringArg(args, "id"));
      case "create_folder": return this.#notebook.createFolder(stringArg(args, "accountId"), stringArg(args, "name"));
      case "rename_folder": return this.#notebook.renameFolder(stringArg(args, "id"), stringArg(args, "name"));
      case "delete_folder": return this.#notebook.deleteFolder(stringArg(args, "id"));
      case "store_media": return this.#media.store({
        noteId: stringArg(args, "noteId"), accountId: stringArg(args, "accountId"), hash: stringArg(args, "hash"),
        mime: stringArg(args, "mime"), mediaType: stringArg(args, "mediaType"), width: numberArg(args, "width"),
        height: numberArg(args, "height"), durationSeconds: numberArg(args, "durationSeconds"),
        originalFilename: stringArg(args, "originalFilename"), bytes: bytesArg(args, "bytes"), thumb: bytesArg(args, "thumb"),
      });
      case "resolve_media": return this.#media.resolve(stringArg(args, "hash"));
      case "ensure_media": return this.#media.ensure(stringArg(args, "noteId"), stringArg(args, "hash"));
      case "delete_media": return this.#media.delete(stringArg(args, "hash"));
      case "save_media": return this.#media.save(stringArg(args, "hash"), stringArg(args, "filename"));
      case "sync_now": {
        const status = await this.#auth.status();
        if (!isSignedIn(status)) return;
        await this.#sync.syncAll();
        return;
      }
      default: throw new Error(`Unknown desktop command '${command}'`);
    }
  }

  close(): void {
    this.#store.close();
  }

  #accounts(): unknown[] {
    return this.#store.db
      .prepare("SELECT id, name, broker, currency, icon FROM accounts_cache ORDER BY name ASC")
      .all();
  }

  #marketWatchlistSymbols(): string[] {
    const rows = this.#store.db
      .prepare(
        `SELECT UPPER(TRIM(symbol)) AS symbol, MAX(open_date) AS last_seen
         FROM journal_entries
         WHERE deleted_at IS NULL AND TRIM(symbol) <> ''
         GROUP BY UPPER(TRIM(symbol))
         ORDER BY last_seen DESC
         LIMIT 8`,
      )
      .all() as Array<{ symbol: string }>;
    return rows.map((row) => row.symbol);
  }

  async #marketQuotes(symbols: string[]): Promise<unknown> {
    const data = (await this.#graphql(MARKET_QUOTES, { symbols })) as { marketQuotes?: unknown };
    if (!data.marketQuotes) throw new Error("missing marketQuotes in response");
    return data.marketQuotes;
  }

  async #playbookStats(): Promise<unknown[]> {
    try {
      const result = (await this.#graphql(
        "query PlaybookStats { playbooks { id winRate cumulativeProfit averageGain averageLoss tradeCount } }",
        {},
      )) as { playbooks?: unknown[] };
      return Array.isArray(result.playbooks) ? result.playbooks : [];
    } catch {
      return [];
    }
  }
}

export function startBackgroundSync(
  engine: SyncEngine,
  options: {
    intervalMs?: number;
    onUpdates?: (noteIds: string[]) => void;
    logger?: Pick<Console, "error">;
    shouldSync?: () => boolean | Promise<boolean>;
  } = {},
): () => void {
  const intervalMs = options.intervalMs ?? 60_000;
  const logger = options.logger ?? console;
  let running = false;
  const run = async () => {
    if (running) return;
    running = true;
    try {
      if (options.shouldSync && !(await options.shouldSync())) return;
      const reports = await engine.syncAll();
      const noteIds = [...new Set(reports.flatMap((report) => report.updatedNotes))];
      if (noteIds.length > 0) options.onUpdates?.(noteIds);
    } catch (error) {
      logger.error("desktop sync:", error);
    } finally {
      running = false;
    }
  };
  void run();
  const timer = setInterval(() => void run(), intervalMs);
  return () => clearInterval(timer);
}

function stringArg(args: Record<string, unknown>, key: string): string {
  const value = args[key];
  if (typeof value !== "string") throw new Error(`missing or invalid argument '${key}'`);
  return value;
}
function numberArg(args: Record<string, unknown>, key: string): number {
  const value = args[key];
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`missing or invalid argument '${key}'`);
  return value;
}
function objectArg(args: Record<string, unknown>, key: string): Record<string, unknown> {
  const value = args[key];
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`missing or invalid argument '${key}'`);
  return value as Record<string, unknown>;
}
function optionalObjectArg(args: Record<string, unknown>, key: string): Record<string, unknown> {
  const value = args[key];
  if (value === undefined || value === null) return {};
  if (typeof value !== "object" || Array.isArray(value)) throw new Error(`invalid argument '${key}'`);
  return value as Record<string, unknown>;
}
function arrayArg(args: Record<string, unknown>, key: string): unknown[] {
  const value = args[key];
  if (!Array.isArray(value)) throw new Error(`missing or invalid argument '${key}'`);
  return value;
}
function stringArrayArg(args: Record<string, unknown>, key: string): string[] {
  const value = arrayArg(args, key);
  if (!value.every((item) => typeof item === "string")) throw new Error(`invalid argument '${key}'`);
  return value;
}
function optionalString(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== "string") throw new Error("invalid optional string argument");
  return value;
}
function isSignedIn(value: unknown): boolean {
  return Boolean(value && typeof value === "object" && (value as { signedIn?: unknown }).signedIn === true);
}
function bytesArg(args: Record<string, unknown>, key: string): Uint8Array | number[] {
  const value = args[key];
  if (value instanceof Uint8Array) return value;
  if (Array.isArray(value) && value.every((item) => Number.isInteger(item) && item >= 0 && item <= 255)) return value as number[];
  if (value && typeof value === "object" && Array.isArray((value as { data?: unknown }).data)) return (value as { data: number[] }).data;
  throw new Error(`missing or invalid byte argument '${key}'`);
}
