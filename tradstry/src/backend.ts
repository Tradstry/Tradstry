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

// --- Playbooks ------------------------------------------------------------

export type Playbook = {
  id: string;
  name: string;
  edgeName: string;
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules: string | null;
  winRate: number;
  cumulativeProfit: number;
  averageGain: number;
  averageLoss: number;
  tradeCount: number;
};

export function playbooks(): Promise<Playbook[]> {
  return invoke("playbooks");
}

export type CreatePlaybookInput = {
  name: string;
  edgeName: string;
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules?: string | null;
};

export type UpdatePlaybookInput = {
  name?: string;
  edgeName?: string;
  entryRules?: string;
  exitRules?: string;
  positionSizingRules?: string;
  additionalRules?: string | null;
  clearAdditionalRules?: boolean;
};

// --- Journal entries (trades) ---------------------------------------------

export type Tag = {
  id: string;
  name: string;
  color: string | null;
  /** Present from `tags()`; null when nested under a JournalEntry. */
  categoryId: string | null;
};

export type TagCategory = {
  id: string;
  name: string;
  role: string | null;
  color: string | null;
  sortOrder: number;
};

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
  status: "profit" | "loss";
  totalPl: number;
  netRoi: number;
  duration: number;
  riskReward: number;
  tradeType: "long" | "short";
  tags: Tag[];
  playbookId: string | null;
  notes: string | null;
};

export function journalEntries(): Promise<JournalEntry[]> {
  return invoke("journal_entries");
}

export function tags(): Promise<Tag[]> {
  return invoke("tags");
}

export function tagCategories(): Promise<TagCategory[]> {
  return invoke("tag_categories");
}

export function createTag(
  categoryId: string,
  name: string,
  color: string | null = null,
): Promise<Tag> {
  return invoke("create_tag", { categoryId, name, color });
}

// --- Tag & category management (for the tag manager) ----------------------

export function createTagCategory(
  name: string,
  color: string | null = null,
): Promise<TagCategory> {
  return invoke("create_tag_category", { name, color });
}

export function renameTagCategory(
  id: string,
  name: string,
): Promise<TagCategory> {
  return invoke("rename_tag_category", { id, name });
}

export function setTagCategoryColor(
  id: string,
  color: string | null,
): Promise<TagCategory> {
  return invoke("set_tag_category_color", { id, color });
}

export function reorderTagCategories(
  order: { id: string; sortOrder: number }[],
): Promise<boolean> {
  return invoke("reorder_tag_categories", { order });
}

export function deleteTagCategory(id: string): Promise<boolean> {
  return invoke("delete_tag_category", { id });
}

export function renameTag(id: string, name: string): Promise<Tag> {
  return invoke("rename_tag", { id, name });
}

export function setTagColor(id: string, color: string | null): Promise<Tag> {
  return invoke("set_tag_color", { id, color });
}

export function deleteTag(id: string): Promise<boolean> {
  return invoke("delete_tag", { id });
}

export function mergeTags(fromId: string, intoId: string): Promise<boolean> {
  return invoke("merge_tags", { fromId, intoId });
}

// --- Advanced analytics (the Analytics page) ------------------------------
// Units: winRate & *Pct are percentages (0..100); netProfit, *Dollars,
// averageGain/Loss, expectancyDollars, equity are USD; *R & profitFactor &
// recoveryFactor & sqn are ratios; *Secs are seconds. Nullable = "—".

export type GroupMetrics = {
  tradeCount: number;
  netProfit: number;
  winRate: number;
  expectancyDollars: number;
  expectancyR: number | null;
  profitFactor: number | null;
};

export type DimensionStat = { key: string; metrics: GroupMetrics };
export type EquityPoint = { closeDate: string; equity: number };
export type RBucket = { label: string; count: number };
export type CleanFlawed = { clean: GroupMetrics; flawed: GroupMetrics };
export type TradesPerDay = { avg: number; max: number; stdev: number | null };
export type CategoryBreakdown = {
  categoryName: string;
  role: string | null;
  tags: DimensionStat[];
};

export type AdvancedAnalytics = {
  tradeCount: number;
  netProfit: number;
  winRate: number;
  expectancyDollars: number;
  expectancyR: number | null;
  rTradeCount: number;
  profitFactor: number | null;
  sqn: number | null;
  averageGain: number;
  averageLoss: number;
  averageGainPct: number;
  averageLossPct: number;
  maxDrawdownDollars: number;
  maxDrawdownPct: number;
  currentDrawdownDollars: number;
  recoveryFactor: number | null;
  longestDrawdownDays: number;
  equityCurve: EquityPoint[];
  startingEquity: number | null;
  accountEquity: number | null;
  avgPlannedR: number | null;
  avgActualR: number | null;
  rDistribution: RBucket[];
  longestWinStreak: number;
  longestLossStreak: number;
  currentStreak: number;
  avgHoldWinnersSecs: number | null;
  avgHoldLosersSecs: number | null;
  monthlyWinRateStdev: number | null;
  tradesPerDay: TradesPerDay;
  bySymbol: DimensionStat[];
  byDayOfWeek: DimensionStat[];
  bySession: DimensionStat[];
  byHolding: DimensionStat[];
  byDirection: DimensionStat[];
  byPositionSize: DimensionStat[];
  byPlaybook: DimensionStat[];
  cleanVsFlawed: CleanFlawed;
  tagBreakdowns: CategoryBreakdown[];
  rangeStart: string | null;
  rangeEnd: string | null;
};

export function advancedAnalytics(
  accountId: string,
  timeFilter: AnalyticsTimeFilter = { range: "ALL" },
): Promise<AdvancedAnalytics> {
  return invoke("advanced_analytics", { accountId, timeFilter });
}

export type CreateJournalEntryInput = {
  accountId: string;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  stopLoss: number;
  symbol: string;
  symbolName?: string | null;
  tradeType: "long" | "short";
  tagIds: string[];
  playbookId?: string | null;
  notes?: string | null;
};

export type UpdateJournalEntryInput = {
  openDate?: string;
  closeDate?: string;
  entryPrice?: number;
  exitPrice?: number;
  positionSize?: number;
  stopLoss?: number;
  symbol?: string;
  symbolName?: string | null;
  tradeType?: "long" | "short";
  tagIds?: string[];
  playbookId?: string | null;
  notes?: string | null;
  clearNotes?: boolean;
  clearPlaybook?: boolean;
};

export function createJournalEntry(
  input: CreateJournalEntryInput,
): Promise<JournalEntry> {
  return invoke("create_journal_entry", { input });
}

export function updateJournalEntry(
  id: string,
  input: UpdateJournalEntryInput,
): Promise<JournalEntry> {
  return invoke("update_journal_entry", { id, input });
}

export function deleteJournalEntry(id: string): Promise<boolean> {
  return invoke("delete_journal_entry", { id });
}

export function createPlaybook(input: CreatePlaybookInput): Promise<Playbook> {
  return invoke("create_playbook", { input });
}

export function updatePlaybook(
  id: string,
  input: UpdatePlaybookInput,
): Promise<Playbook> {
  return invoke("update_playbook", { id, input });
}

export function deletePlaybook(id: string): Promise<boolean> {
  return invoke("delete_playbook", { id });
}
