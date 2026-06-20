export const ANALYTICS_RANGES = [
  "TODAY",
  "LAST_7_DAYS",
  "LAST_1_MONTH",
  "LAST_3_MONTHS",
  "LAST_6_MONTHS",
  "YEAR_TO_DATE",
  "LAST_1_YEAR",
  "ALL",
  "CUSTOM",
] as const;

export type AnalyticsRange = (typeof ANALYTICS_RANGES)[number];

export interface AnalyticsTimeFilterInput {
  range: AnalyticsRange;
  startDate?: string | null;
  endDate?: string | null;
}

export interface TradeOutcome {
  symbol: string;
  symbolName: string;
  amount: number;
}

export interface JournalAnalytics {
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
  /** Resolved ET calendar bounds of the active range (YYYY-MM-DD); null for "all". */
  rangeStart: string | null;
  rangeEnd: string | null;
}

export interface CalendarDaySummary {
  date: string;
  profit: number;
  tradeCount: number;
  winRate: number;
}

export interface CalendarWeekSummary {
  weekIndex: number;
  weekStart: string;
  weekEnd: string;
  profit: number;
  tradeCount: number;
  tradingDays: number;
}

export interface CalendarAnalytics {
  year: number;
  month: number;
  monthProfit: number;
  tradeCount: number;
  tradingDays: number;
  gridStart: string;
  gridEnd: string;
  days: CalendarDaySummary[];
  weeks: CalendarWeekSummary[];
}

// ---------------------------------------------------------------------------
// Advanced analytics (the dedicated Analytics page). Mirrors the backend
// `AdvancedAnalytics` payload. Fields are added to the GraphQL query
// section-by-section as the page grows; this type is the full contract.
// ---------------------------------------------------------------------------

export interface GroupMetrics {
  tradeCount: number;
  netProfit: number;
  winRate: number; // 0..100
  expectancyDollars: number;
  expectancyR: number | null;
  profitFactor: number | null;
}

export interface DimensionStat {
  key: string;
  metrics: GroupMetrics;
}

export interface EquityPoint {
  closeDate: string;
  equity: number;
}

export interface RBucket {
  label: string;
  count: number;
}

export interface CleanFlawed {
  clean: GroupMetrics;
  flawed: GroupMetrics;
}

export interface TradesPerDay {
  avg: number;
  max: number;
  stdev: number | null;
}

export interface CategoryBreakdown {
  categoryName: string;
  role: string | null;
  tags: DimensionStat[];
}

export interface AdvancedAnalytics {
  // Headline
  tradeCount: number;
  netProfit: number;
  winRate: number; // 0..100
  expectancyDollars: number;
  expectancyR: number | null;
  rTradeCount: number;
  profitFactor: number | null;
  sqn: number | null;
  averageGain: number;
  averageLoss: number;
  averageGainPct: number;
  averageLossPct: number;
  // Risk / drawdown
  maxDrawdownDollars: number;
  maxDrawdownPct: number;
  currentDrawdownDollars: number;
  recoveryFactor: number | null;
  longestDrawdownDays: number;
  equityCurve: EquityPoint[];
  startingEquity: number | null;
  accountEquity: number | null;
  // R-multiples
  avgPlannedR: number | null;
  avgActualR: number | null;
  rDistribution: RBucket[];
  // Streaks & consistency
  longestWinStreak: number;
  longestLossStreak: number;
  currentStreak: number;
  avgHoldWinnersSecs: number | null;
  avgHoldLosersSecs: number | null;
  monthlyWinRateStdev: number | null;
  tradesPerDay: TradesPerDay;
  // Breakdowns
  bySymbol: DimensionStat[];
  byDayOfWeek: DimensionStat[];
  bySession: DimensionStat[];
  byHolding: DimensionStat[];
  byDirection: DimensionStat[];
  byPositionSize: DimensionStat[];
  byPlaybook: DimensionStat[];
  // Behavioral
  cleanVsFlawed: CleanFlawed;
  tagBreakdowns: CategoryBreakdown[];
  /** Resolved ET calendar bounds of the active range (YYYY-MM-DD); null for "all". */
  rangeStart: string | null;
  rangeEnd: string | null;
}
