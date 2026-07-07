import { invoke } from "@tauri-apps/api/core";

// Thin typed bindings to the Rust commands. All GraphQL/query logic lives in
// src-tauri; the frontend only invokes named commands.

export type AnalyticsRange =
  | "TODAY"
  | "LAST_7_DAYS"
  | "LAST_1_MONTH"
  | "LAST_3_MONTHS"
  | "LAST_6_MONTHS"
  | "YEAR_TO_DATE"
  | "LAST_1_YEAR"
  | "ALL"
  | "CUSTOM";

export type AnalyticsTimeFilter = {
  range: AnalyticsRange;
  startDate?: string | null;
  endDate?: string | null;
};

export type TradeOutcome = {
  symbol: string;
  symbolName: string | null;
  amount: number;
};

export type JournalAnalytics = {
  winRate: number;
  cumulativeProfit: number;
  averageRiskToReward: number;
  averageGain: number;
  averageLoss: number;
  averageGainPct: number;
  averageLossPct: number;
  profitFactor: number | null;
  biggestWin: TradeOutcome | null;
  biggestLoss: TradeOutcome | null;
  rangeStart: string | null;
  rangeEnd: string | null;
};

export function journalAnalytics(
  accountId: string,
  timeFilter: AnalyticsTimeFilter = { range: "ALL" },
): Promise<JournalAnalytics> {
  return invoke("journal_analytics", { accountId, timeFilter });
}

// --- Accounts -------------------------------------------------------------

export type Account = {
  id: string;
  name: string;
  broker: string | null;
  currency: string | null;
  icon: string | null;
};

export function accounts(): Promise<Account[]> {
  return invoke("accounts");
}

// --- Calendar analytics ---------------------------------------------------

export type CalendarDay = {
  date: string;
  profit: number;
  tradeCount: number;
  winRate: number;
};

export type CalendarWeek = {
  weekIndex: number;
  weekStart: string;
  weekEnd: string;
  profit: number;
  tradeCount: number;
  tradingDays: number;
};

export type CalendarAnalytics = {
  year: number;
  month: number;
  monthProfit: number;
  tradeCount: number;
  tradingDays: number;
  gridStart: string;
  gridEnd: string;
  days: CalendarDay[];
  weeks: CalendarWeek[];
};

export function calendarAnalytics(
  accountId: string,
  year: number,
  month: number,
): Promise<CalendarAnalytics> {
  return invoke("calendar_analytics", { accountId, year, month });
}
