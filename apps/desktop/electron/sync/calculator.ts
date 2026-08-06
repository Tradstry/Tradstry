import type { DesktopDatabase } from "./database.ts";
import { transaction } from "./database.ts";
import { enqueueMutation, uuidV7 } from "./mutations.ts";

export type CalculatorRule = {
  id: string;
  accountId: string;
  accountBalance: number;
  accountRisk: number;
  maxStopLossPct: number;
};

export type CalculatorPlanInput = {
  symbol: string;
  positionType: string;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  totalShares: number;
  positionValue: number;
  tranchesJson: string;
  notes?: string | null;
};

export type Tranche = {
  id: string;
  percent: number;
  shares: number;
  targetPrice: number;
  status: string;
  filledAt: string | null;
};

export type CalculatorPlan = Omit<CalculatorPlanInput, "tranchesJson"> & {
  id: string;
  status: string;
  tranches: Tranche[];
  notes: string | null;
  createdAt: string;
};

export type CalculatorHistoryInput = {
  symbol: string;
  positionType: string;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  shares: number;
  positionValue: number;
  accountPct: number;
  stopLossPct: number;
};

export type CalculatorHistory = CalculatorHistoryInput & { id: string; createdAt: string };

type PlanRow = {
  id: string; symbol: string; position_type: string; entry_price: number; stop_loss: number;
  account_balance: number; account_risk: number; total_shares: number; position_value: number;
  status: string; tranches_json: string; notes: string | null; created_at: string;
};
type HistoryRow = {
  id: string; symbol: string; position_type: string; entry_price: number; stop_loss: number;
  account_balance: number; account_risk: number; shares: number; position_value: number;
  account_pct: number; stop_loss_pct: number; created_at: string;
};

const PLAN_COLUMNS = "id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, total_shares, position_value, status, tranches_json, notes, created_at";
const HISTORY_COLUMNS = "id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, shares, position_value, account_pct, stop_loss_pct, created_at";

export class CalculatorRepository {
  readonly #store: DesktopDatabase;

  constructor(store: DesktopDatabase) {
    this.#store = store;
  }

  rule(accountId: string): CalculatorRule | null {
    const row = this.#store.db
      .prepare("SELECT id, account_id, account_balance, account_risk, max_stop_loss_pct FROM calc_rules WHERE account_id = ? AND deleted_at IS NULL")
      .get(accountId) as Record<string, unknown> | undefined;
    return row ? ruleView(row) : null;
  }

  upsertRule(input: Omit<CalculatorRule, "id">): CalculatorRule {
    const existing = this.#store.db.prepare("SELECT id FROM calc_rules WHERE account_id = ?").get(input.accountId) as
      | { id: string }
      | undefined;
    const id = existing?.id ?? uuidV7();
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          `INSERT INTO calc_rules (account_id, id, account_balance, account_risk, max_stop_loss_pct, hlc, sync_state)
           VALUES (?, ?, ?, ?, ?, ?, 'pending')
           ON CONFLICT(account_id) DO UPDATE SET account_balance = excluded.account_balance,
             account_risk = excluded.account_risk, max_stop_loss_pct = excluded.max_stop_loss_pct,
             hlc = excluded.hlc, deleted_at = NULL, sync_state = 'pending'`,
        )
        .run(input.accountId, id, input.accountBalance, input.accountRisk, input.maxStopLossPct, stamp);
      enqueueMutation(this.#store.db, "upsertPositionCalculatorRule", { id, ...input }, stamp);
    });
    return { id, ...input };
  }

  plans(): CalculatorPlan[] {
    return (this.#store.db
      .prepare(`SELECT ${PLAN_COLUMNS} FROM calc_plans WHERE deleted_at IS NULL ORDER BY created_at DESC`)
      .all() as PlanRow[]).map(planView);
  }

  createPlan(input: CalculatorPlanInput): CalculatorPlan {
    const id = uuidV7();
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          `INSERT INTO calc_plans
           (id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk,
            total_shares, position_value, status, tranches_json, notes, hlc, sync_state)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, 'pending')`,
        )
        .run(id, input.symbol, input.positionType, input.entryPrice, input.stopLoss, input.accountBalance, input.accountRisk, input.totalShares, input.positionValue, input.tranchesJson, input.notes ?? null, stamp);
      enqueueMutation(this.#store.db, "createPositionCalculatorPlan", { id, ...input, notes: input.notes ?? null }, stamp);
    });
    return planView(this.#requiredPlan(id));
  }

  updatePlan(id: string, input: { status?: string; tranchesJson?: string; notes?: string | null; clearNotes?: boolean }): CalculatorPlan {
    const current = this.#requiredPlan(id);
    const status = input.status ?? current.status;
    const tranchesJson = input.tranchesJson ?? current.tranches_json;
    const notes = input.clearNotes ? null : input.notes ?? current.notes;
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("UPDATE calc_plans SET status = ?, tranches_json = ?, notes = ?, hlc = ?, sync_state = 'pending' WHERE id = ?")
        .run(status, tranchesJson, notes, stamp, id);
      enqueueMutation(this.#store.db, "updatePositionCalculatorPlan", { id, status, tranchesJson, notes }, stamp);
    });
    return planView(this.#requiredPlan(id));
  }

  deletePlan(id: string): boolean {
    this.#delete("calc_plans", id, "deletePositionCalculatorPlan");
    return true;
  }

  history(): CalculatorHistory[] {
    return (this.#store.db
      .prepare(`SELECT ${HISTORY_COLUMNS} FROM calc_history WHERE deleted_at IS NULL ORDER BY created_at DESC`)
      .all() as HistoryRow[]).map(historyView);
  }

  createHistory(input: CalculatorHistoryInput): CalculatorHistory {
    const id = uuidV7();
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          `INSERT INTO calc_history
           (id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk,
            shares, position_value, account_pct, stop_loss_pct, hlc, sync_state)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')`,
        )
        .run(id, input.symbol, input.positionType, input.entryPrice, input.stopLoss, input.accountBalance, input.accountRisk, input.shares, input.positionValue, input.accountPct, input.stopLossPct, stamp);
      enqueueMutation(this.#store.db, "createPositionCalculatorHistory", { id, ...input }, stamp);
    });
    const row = this.#store.db.prepare(`SELECT ${HISTORY_COLUMNS} FROM calc_history WHERE id = ?`).get(id) as HistoryRow;
    return historyView(row);
  }

  deleteHistory(id: string): boolean {
    this.#delete("calc_history", id, "deletePositionCalculatorHistory");
    return true;
  }

  #requiredPlan(id: string): PlanRow {
    const row = this.#store.db.prepare(`SELECT ${PLAN_COLUMNS} FROM calc_plans WHERE id = ? AND deleted_at IS NULL`).get(id) as
      | PlanRow
      | undefined;
    if (!row) throw new Error("plan not found");
    return row;
  }

  #delete(table: "calc_plans" | "calc_history", id: string, mutation: string): void {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(`UPDATE ${table} SET deleted_at = datetime('now'), hlc = ?, sync_state = 'pending' WHERE id = ?`)
        .run(stamp, id);
      enqueueMutation(this.#store.db, mutation, { id }, stamp);
    });
  }
}

function ruleView(row: Record<string, unknown>): CalculatorRule {
  return {
    id: String(row.id), accountId: String(row.account_id), accountBalance: Number(row.account_balance),
    accountRisk: Number(row.account_risk), maxStopLossPct: Number(row.max_stop_loss_pct),
  };
}

function planView(row: PlanRow): CalculatorPlan {
  let tranches: Tranche[] = [];
  try {
    const parsed: unknown = JSON.parse(row.tranches_json);
    if (Array.isArray(parsed)) tranches = parsed as Tranche[];
  } catch {}
  return {
    id: row.id, symbol: row.symbol, positionType: row.position_type, entryPrice: row.entry_price,
    stopLoss: row.stop_loss, accountBalance: row.account_balance, accountRisk: row.account_risk,
    totalShares: row.total_shares, positionValue: row.position_value, status: row.status,
    tranches, notes: row.notes, createdAt: row.created_at,
  };
}

function historyView(row: HistoryRow): CalculatorHistory {
  return {
    id: row.id, symbol: row.symbol, positionType: row.position_type, entryPrice: row.entry_price,
    stopLoss: row.stop_loss, accountBalance: row.account_balance, accountRisk: row.account_risk,
    shares: row.shares, positionValue: row.position_value, accountPct: row.account_pct,
    stopLossPct: row.stop_loss_pct, createdAt: row.created_at,
  };
}
