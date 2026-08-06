import type { DesktopDatabase } from "./database.ts";
import { transaction } from "./database.ts";
import { deriveMetrics } from "./derive.ts";
import { enqueueMutation, uuidV7 } from "./mutations.ts";

export type PlaybookInput = {
  name: string;
  edgeName?: string;
  entryRules?: string;
  exitRules?: string;
  positionSizingRules?: string;
  additionalRules?: string | null;
  clearAdditionalRules?: boolean;
};

export type Playbook = Required<Omit<PlaybookInput, "clearAdditionalRules" | "additionalRules">> & {
  id: string;
  additionalRules: string | null;
  winRate: number;
  cumulativeProfit: number;
  averageGain: number;
  averageLoss: number;
  tradeCount: number;
};

export type Tag = { id: string; name: string; color: string | null; categoryId: string | null };

export type JournalEntry = {
  id: string;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  stopLoss: number;
  symbol: string;
  symbolName: string;
  status: string;
  totalPl: number;
  netRoi: number;
  duration: number;
  riskReward: number;
  tradeType: string;
  tags: Tag[];
  playbookId: string | null;
  notes: string | null;
};

export type JournalInput = {
  accountId: string;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  stopLoss?: number | null;
  symbol: string;
  symbolName?: string;
  tradeType: string;
  playbookId?: string | null;
  notes?: string | null;
  broke30MinRule?: boolean | null;
  preTradeConviction?: number | null;
  marketRegime?: string | null;
  isPlannedPreMarket?: boolean | null;
  revengeTrade?: boolean | null;
  ruleAdherenceScore?: number | null;
  tagIds?: string[];
  violatedPrincipleIds?: string[];
  clearNotes?: boolean;
  clearPlaybook?: boolean;
};

type JournalRow = {
  id: string;
  account_id: string;
  open_date: string;
  close_date: string;
  entry_price: number;
  exit_price: number;
  position_size: number;
  stop_loss: number | null;
  symbol: string;
  symbol_name: string;
  status: string;
  total_pl: number;
  net_roi: number;
  duration: number;
  risk_reward: number | null;
  trade_type: string;
  playbook_id: string | null;
  notes: string | null;
  broke_30min_rule: number | null;
  pre_trade_conviction: number | null;
  market_regime: string | null;
  is_planned_pre_market: number | null;
  revenge_trade: number | null;
  rule_adherence_score: number | null;
  tag_ids: string;
  violated_principle_ids: string;
  created_at: string;
};

type JournalWrite = Omit<JournalInput, "clearNotes" | "clearPlaybook"> & { id: string };

const JOURNAL_COLUMNS = `id, account_id, open_date, close_date, entry_price, exit_price,
  position_size, stop_loss, symbol, symbol_name, status, total_pl, net_roi, duration,
  risk_reward, trade_type, playbook_id, notes, broke_30min_rule, pre_trade_conviction,
  market_regime, is_planned_pre_market, revenge_trade, rule_adherence_score, tag_ids,
  violated_principle_ids, created_at`;

export class TradingRepository {
  readonly #store: DesktopDatabase;

  constructor(store: DesktopDatabase) {
    this.#store = store;
  }

  playbooks(): Playbook[] {
    const rows = this.#store.db
      .prepare(
        `SELECT id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules
         FROM playbooks WHERE deleted_at IS NULL ORDER BY name ASC`,
      )
      .all() as Array<Record<string, unknown>>;
    return rows.map(playbookFromRow);
  }

  createPlaybook(input: PlaybookInput): Playbook {
    const id = uuidV7();
    const row = normalizePlaybook(id, input);
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          `INSERT INTO playbooks
           (id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules, hlc, sync_state)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending')`,
        )
        .run(id, row.name, row.edgeName, row.entryRules, row.exitRules, row.positionSizingRules, row.additionalRules, stamp);
      enqueueMutation(this.#store.db, "createPlaybook", playbookArgs(row), stamp);
    });
    return withEmptyStats(row);
  }

  updatePlaybook(id: string, input: Partial<PlaybookInput>): Playbook {
    const current = this.#findPlaybook(id);
    if (!current) throw new Error("playbook not found");
    const additionalRules = input.clearAdditionalRules
      ? null
      : input.additionalRules !== undefined
        ? input.additionalRules || null
        : current.additionalRules;
    const row = {
      id,
      name: input.name ?? current.name,
      edgeName: input.edgeName ?? current.edgeName,
      entryRules: input.entryRules ?? current.entryRules,
      exitRules: input.exitRules ?? current.exitRules,
      positionSizingRules: input.positionSizingRules ?? current.positionSizingRules,
      additionalRules,
    };
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          `UPDATE playbooks SET name = ?, edge_name = ?, entry_rules = ?, exit_rules = ?,
           position_sizing_rules = ?, additional_rules = ?, hlc = ?, sync_state = 'pending' WHERE id = ?`,
        )
        .run(row.name, row.edgeName, row.entryRules, row.exitRules, row.positionSizingRules, row.additionalRules, stamp, id);
      enqueueMutation(this.#store.db, "updatePlaybook", playbookArgs(row), stamp);
    });
    return withEmptyStats(row);
  }

  deletePlaybook(id: string): boolean {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("UPDATE playbooks SET deleted_at = datetime('now'), hlc = ?, sync_state = 'pending' WHERE id = ?")
        .run(stamp, id);
      enqueueMutation(this.#store.db, "deletePlaybook", { id }, stamp);
    });
    return true;
  }

  journalEntries(accountId: string): JournalEntry[] {
    const rows = this.#store.db
      .prepare(`SELECT ${JOURNAL_COLUMNS} FROM journal_entries WHERE account_id = ? AND deleted_at IS NULL ORDER BY open_date DESC`)
      .all(accountId) as JournalRow[];
    return rows.map((row) => this.#journalView(row));
  }

  createJournalEntry(input: JournalInput): JournalEntry {
    const write = normalizeJournal(uuidV7(), input);
    this.#writeJournal("create", write);
    const row = this.#findJournal(write.id);
    if (!row) throw new Error("journal entry not found after create");
    return this.#journalView(row);
  }

  updateJournalEntry(id: string, input: Partial<JournalInput>): JournalEntry {
    const current = this.#findJournal(id);
    if (!current) throw new Error("journal entry not found");
    const currentWrite = journalWriteFromRow(current);
    const merged: JournalWrite = {
      ...currentWrite,
      ...withoutUndefined(input),
      id,
      accountId: current.account_id,
      notes: input.clearNotes ? null : input.notes ?? current.notes,
      playbookId: input.clearPlaybook ? null : input.playbookId ?? current.playbook_id,
      stopLoss: input.stopLoss ?? current.stop_loss,
      tagIds: input.tagIds ?? stringArray(current.tag_ids),
      violatedPrincipleIds: input.violatedPrincipleIds ?? stringArray(current.violated_principle_ids),
    };
    delete (merged as Partial<JournalInput>).clearNotes;
    delete (merged as Partial<JournalInput>).clearPlaybook;
    this.#writeJournal("update", merged);
    const row = this.#findJournal(id);
    if (!row) throw new Error("journal entry not found after update");
    return this.#journalView(row);
  }

  deleteJournalEntry(id: string): boolean {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("UPDATE journal_entries SET deleted_at = datetime('now'), hlc = ?, sync_state = 'pending' WHERE id = ?")
        .run(stamp, id);
      enqueueMutation(this.#store.db, "deleteJournalEntry", { id }, stamp);
    });
    return true;
  }

  #findPlaybook(id: string): ReturnType<typeof normalizePlaybook> | null {
    const row = this.#store.db
      .prepare(
        `SELECT id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules
         FROM playbooks WHERE id = ? AND deleted_at IS NULL`,
      )
      .get(id) as Record<string, unknown> | undefined;
    return row ? playbookBaseFromRow(row) : null;
  }

  #findJournal(id: string): JournalRow | null {
    return (this.#store.db
      .prepare(`SELECT ${JOURNAL_COLUMNS} FROM journal_entries WHERE id = ? AND deleted_at IS NULL`)
      .get(id) as JournalRow | undefined) ?? null;
  }

  #writeJournal(kind: "create" | "update", write: JournalWrite): void {
    const derived = deriveMetrics(
      write.entryPrice,
      write.exitPrice,
      write.tradeType,
      write.openDate,
      write.closeDate,
      write.stopLoss ?? null,
    );
    const stamp = this.#store.hlc.now();
    const values = [
      write.openDate, write.closeDate, write.entryPrice, write.exitPrice, write.positionSize,
      write.stopLoss ?? null, write.symbol, write.symbolName ?? "", derived.status, derived.totalPl,
      derived.netRoi, derived.duration, derived.riskReward, write.tradeType, write.playbookId ?? null,
      write.notes ?? null, boolValue(write.broke30MinRule), write.preTradeConviction ?? null,
      write.marketRegime ?? null, boolValue(write.isPlannedPreMarket), boolValue(write.revengeTrade),
      write.ruleAdherenceScore ?? null, JSON.stringify(write.tagIds ?? []),
      JSON.stringify(write.violatedPrincipleIds ?? []), stamp,
    ] as const;
    transaction(this.#store.db, () => {
      if (kind === "create") {
        this.#store.db
          .prepare(
            `INSERT INTO journal_entries
             (id, account_id, open_date, close_date, entry_price, exit_price, position_size,
              stop_loss, symbol, symbol_name, status, total_pl, net_roi, duration, risk_reward,
              trade_type, playbook_id, notes, broke_30min_rule, pre_trade_conviction, market_regime,
              is_planned_pre_market, revenge_trade, rule_adherence_score, tag_ids,
              violated_principle_ids, hlc, sync_state)
             VALUES (?, ?, ${Array.from({ length: 25 }, () => "?").join(", ")}, 'pending')`,
          )
          .run(write.id, write.accountId, ...values);
      } else {
        const columns = [
          "open_date", "close_date", "entry_price", "exit_price", "position_size", "stop_loss",
          "symbol", "symbol_name", "status", "total_pl", "net_roi", "duration", "risk_reward",
          "trade_type", "playbook_id", "notes", "broke_30min_rule", "pre_trade_conviction",
          "market_regime", "is_planned_pre_market", "revenge_trade", "rule_adherence_score",
          "tag_ids", "violated_principle_ids", "hlc",
        ];
        this.#store.db
          .prepare(`UPDATE journal_entries SET ${columns.map((column) => `${column} = ?`).join(", ")}, sync_state = 'pending' WHERE id = ?`)
          .run(...values, write.id);
      }
      enqueueMutation(this.#store.db, kind === "create" ? "createJournalEntry" : "updateJournalEntry", journalArgs(write), stamp);
    });
  }

  #journalView(row: JournalRow): JournalEntry {
    const ids = stringArray(row.tag_ids);
    const tags = ids.flatMap((id) => {
      const tag = this.#store.db
        .prepare("SELECT id, name, color, category_id AS categoryId FROM tags_cache WHERE id = ? AND deleted_at IS NULL")
        .get(id) as Tag | undefined;
      return tag ? [tag] : [];
    });
    return {
      id: row.id,
      openDate: row.open_date,
      closeDate: row.close_date,
      entryPrice: row.entry_price,
      exitPrice: row.exit_price,
      positionSize: row.position_size,
      stopLoss: row.stop_loss ?? 0,
      symbol: row.symbol,
      symbolName: row.symbol_name,
      status: row.status,
      totalPl: row.total_pl,
      netRoi: row.net_roi,
      duration: row.duration,
      riskReward: row.risk_reward ?? 0,
      tradeType: row.trade_type,
      tags,
      playbookId: row.playbook_id,
      notes: row.notes,
    };
  }
}

function normalizePlaybook(id: string, input: PlaybookInput) {
  return {
    id,
    name: input.name,
    edgeName: input.edgeName ?? "",
    entryRules: input.entryRules ?? "",
    exitRules: input.exitRules ?? "",
    positionSizingRules: input.positionSizingRules ?? "",
    additionalRules: input.additionalRules ?? null,
  };
}

function playbookBaseFromRow(row: Record<string, unknown>): ReturnType<typeof normalizePlaybook> {
  return {
    id: String(row.id),
    name: String(row.name),
    edgeName: String(row.edge_name),
    entryRules: String(row.entry_rules),
    exitRules: String(row.exit_rules),
    positionSizingRules: String(row.position_sizing_rules),
    additionalRules: typeof row.additional_rules === "string" ? row.additional_rules : null,
  };
}

function playbookFromRow(row: Record<string, unknown>): Playbook {
  return withEmptyStats(playbookBaseFromRow(row));
}

function withEmptyStats(row: ReturnType<typeof normalizePlaybook>): Playbook {
  return { ...row, winRate: 0, cumulativeProfit: 0, averageGain: 0, averageLoss: 0, tradeCount: 0 };
}

function playbookArgs(row: ReturnType<typeof normalizePlaybook>): Record<string, unknown> {
  return { ...row };
}

function normalizeJournal(id: string, input: JournalInput): JournalWrite {
  return {
    id,
    accountId: required(input.accountId, "accountId"),
    openDate: required(input.openDate, "openDate"),
    closeDate: required(input.closeDate, "closeDate"),
    entryPrice: requiredNumber(input.entryPrice, "entryPrice"),
    exitPrice: requiredNumber(input.exitPrice, "exitPrice"),
    positionSize: requiredNumber(input.positionSize, "positionSize"),
    stopLoss: input.stopLoss ?? null,
    symbol: required(input.symbol, "symbol"),
    symbolName: input.symbolName ?? "",
    tradeType: required(input.tradeType, "tradeType"),
    playbookId: input.playbookId ?? null,
    notes: input.notes ?? null,
    broke30MinRule: input.broke30MinRule ?? null,
    preTradeConviction: input.preTradeConviction ?? null,
    marketRegime: input.marketRegime ?? null,
    isPlannedPreMarket: input.isPlannedPreMarket ?? null,
    revengeTrade: input.revengeTrade ?? null,
    ruleAdherenceScore: input.ruleAdherenceScore ?? null,
    tagIds: input.tagIds ?? [],
    violatedPrincipleIds: input.violatedPrincipleIds ?? [],
  };
}

function journalWriteFromRow(row: JournalRow): JournalWrite {
  return {
    id: row.id,
    accountId: row.account_id,
    openDate: row.open_date,
    closeDate: row.close_date,
    entryPrice: row.entry_price,
    exitPrice: row.exit_price,
    positionSize: row.position_size,
    stopLoss: row.stop_loss,
    symbol: row.symbol,
    symbolName: row.symbol_name,
    tradeType: row.trade_type,
    playbookId: row.playbook_id,
    notes: row.notes,
    broke30MinRule: nullableBool(row.broke_30min_rule),
    preTradeConviction: row.pre_trade_conviction,
    marketRegime: row.market_regime,
    isPlannedPreMarket: nullableBool(row.is_planned_pre_market),
    revengeTrade: nullableBool(row.revenge_trade),
    ruleAdherenceScore: row.rule_adherence_score,
    tagIds: stringArray(row.tag_ids),
    violatedPrincipleIds: stringArray(row.violated_principle_ids),
  };
}

function journalArgs(write: JournalWrite): Record<string, unknown> {
  return {
    id: write.id,
    accountId: write.accountId,
    openDate: write.openDate,
    closeDate: write.closeDate,
    entryPrice: write.entryPrice,
    exitPrice: write.exitPrice,
    positionSize: write.positionSize,
    stopLoss: write.stopLoss ?? null,
    symbol: write.symbol,
    symbolName: write.symbolName ?? "",
    tradeType: write.tradeType,
    playbookId: write.playbookId ?? null,
    notes: write.notes ?? null,
    broke30MinRule: write.broke30MinRule ?? null,
    preTradeConviction: write.preTradeConviction ?? null,
    marketRegime: write.marketRegime ?? null,
    isPlannedPreMarket: write.isPlannedPreMarket ?? null,
    revengeTrade: write.revengeTrade ?? null,
    ruleAdherenceScore: write.ruleAdherenceScore ?? null,
    tagIds: write.tagIds ?? [],
    violatedPrincipleIds: write.violatedPrincipleIds ?? [],
  };
}

function boolValue(value: boolean | null | undefined): number | null {
  return value == null ? null : Number(value);
}

function nullableBool(value: number | null): boolean | null {
  return value === null ? null : Boolean(value);
}

function stringArray(value: string): string[] {
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string") : [];
  } catch {
    return [];
  }
}

function required(value: string | undefined, key: string): string {
  if (value === undefined) throw new Error(`${key} is required`);
  return value;
}

function requiredNumber(value: number | undefined, key: string): number {
  if (value === undefined) throw new Error(`${key} is required`);
  return value;
}

function withoutUndefined<T extends object>(value: T): Partial<T> {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined)) as Partial<T>;
}
