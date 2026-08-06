import { DEFAULT_NOTE_DOC, toBase64 } from "@tradstry/notebook-core";
import { jsonToUpdate } from "@tradstry/notebook-core/seed";

const invoke = window.tradstry.invoke;

// Thin typed bindings to the Electron main-process service. Database and
// network logic stays outside the sandboxed renderer and crosses preload only
// through named commands.

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

export function journalEntries(accountId: string): Promise<JournalEntry[]> {
  return invoke("journal_entries", { accountId });
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

/** Behavioral / discipline aggregates. `avgRuleAdherence`/`avgConviction` are
 * 1..5 scale means (nullable when no trade recorded a value). */
export type Discipline = {
  flawedTradeCount: number;
  mistakeCost: number;
  avgRuleAdherence: number | null;
  avgConviction: number | null;
  revengeTradeCount: number;
  broke30MinCount: number;
  tradesWithViolations: number;
  totalViolations: number;
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
  discipline: Discipline;
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

export type PlaybookStat = {
  id: string;
  winRate: number;
  cumulativeProfit: number;
  averageGain: number;
  averageLoss: number;
  tradeCount: number;
};

/** Best-effort online overlay of derived stats; resolves to [] when offline. */
export function playbookStats(): Promise<PlaybookStat[]> {
  return invoke("playbook_stats");
}

// --- Trading principles (local-first; violation stats computed on-device) -

export type Principle = {
  id: string;
  accountId: string;
  playbookId: string | null;
  evidenceNoteId: string | null;
  evidenceNoteTitle: string | null;
  title: string;
  theRule: string;
  why: string;
  intervention: string | null;
  priority: number;
  isActive: boolean;
  createdAt: string;
};

/** `violationCount` counts every trade naming this principle (winners, losers,
 * and breakeven); `violatedWinRate`'s denominator excludes breakeven trades. */
export type PrincipleWithStats = Principle & {
  violationCount: number;
  violatedCumulativeProfit: number;
  violatedCumulativeRoi: number;
  violatedWinRate: number;
};

export type CreatePrincipleInput = {
  accountId: string;
  title: string;
  theRule: string;
  why: string;
  intervention?: string | null;
  playbookId?: string | null;
  evidenceNoteId?: string | null;
  isActive?: boolean;
};

export type UpdatePrincipleInput = {
  title?: string;
  theRule?: string;
  why?: string;
  intervention?: string | null;
  clearIntervention?: boolean;
  playbookId?: string | null;
  clearPlaybook?: boolean;
  evidenceNoteId?: string | null;
  clearEvidenceNote?: boolean;
  isActive?: boolean;
};

export function principles(accountId: string): Promise<PrincipleWithStats[]> {
  return invoke("principles", { accountId });
}

export function createPrinciple(
  input: CreatePrincipleInput,
): Promise<PrincipleWithStats> {
  return invoke("create_principle", { input });
}

export function updatePrinciple(
  id: string,
  input: UpdatePrincipleInput,
): Promise<PrincipleWithStats> {
  return invoke("update_principle", { id, input });
}

export function deletePrinciple(id: string): Promise<boolean> {
  return invoke("delete_principle", { id });
}

/** `orderedIds`: the full absolute ordering for every principle in the account. */
export function reorderPrinciples(orderedIds: string[]): Promise<boolean> {
  return invoke("reorder_principles", { orderedIds });
}

// --- Position calculator (local-first; rules are per-account) -------------

export type PositionCalculatorRule = {
  id: string;
  accountId: string;
  accountBalance: number;
  accountRisk: number;
  maxStopLossPct: number;
};

export type UpsertPositionCalculatorRuleInput = {
  accountId: string;
  accountBalance: number;
  accountRisk: number;
  maxStopLossPct: number;
};

export type Tranche = {
  id: string;
  percent: number;
  shares: number;
  targetPrice: number;
  status: string;
  filledAt: string | null;
};

export type PositionCalculatorPlan = {
  id: string;
  symbol: string;
  positionType: string;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  totalShares: number;
  positionValue: number;
  status: string;
  tranches: Tranche[];
  notes: string | null;
  createdAt: string;
};

/** `tranchesJson` is a pre-serialized `Tranche[]` (minus server-only fields
 * like `filledAt`, which the caller leaves out on create); it travels
 * end-to-end as an opaque string — this layer never decodes it except in the
 * return value. */
export type CreatePositionCalculatorPlanInput = {
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

export type UpdatePositionCalculatorPlanInput = {
  status?: string;
  tranchesJson?: string;
  notes?: string | null;
  clearNotes?: boolean;
};

export type PositionCalculatorHistoryEntry = {
  id: string;
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
  createdAt: string;
};

export type CreatePositionCalculatorHistoryInput = {
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

export function positionCalculatorRule(
  accountId: string,
): Promise<PositionCalculatorRule | null> {
  return invoke("position_calculator_rule", { accountId });
}

export function upsertPositionCalculatorRule(
  input: UpsertPositionCalculatorRuleInput,
): Promise<PositionCalculatorRule> {
  return invoke("upsert_position_calculator_rule", { input });
}

export function positionCalculatorPlans(): Promise<PositionCalculatorPlan[]> {
  return invoke("position_calculator_plans");
}

export function createPositionCalculatorPlan(
  input: CreatePositionCalculatorPlanInput,
): Promise<PositionCalculatorPlan> {
  return invoke("create_position_calculator_plan", { input });
}

export function updatePositionCalculatorPlan(
  id: string,
  input: UpdatePositionCalculatorPlanInput,
): Promise<PositionCalculatorPlan> {
  return invoke("update_position_calculator_plan", { id, input });
}

export function deletePositionCalculatorPlan(id: string): Promise<boolean> {
  return invoke("delete_position_calculator_plan", { id });
}

export function positionCalculatorHistory(): Promise<
  PositionCalculatorHistoryEntry[]
> {
  return invoke("position_calculator_history");
}

export function createPositionCalculatorHistory(
  input: CreatePositionCalculatorHistoryInput,
): Promise<PositionCalculatorHistoryEntry> {
  return invoke("create_position_calculator_history", { input });
}

export function deletePositionCalculatorHistory(id: string): Promise<boolean> {
  return invoke("delete_position_calculator_history", { id });
}

// --- Notebook (local-first; reads never touch the network) ----------------

export type Note = {
  id: string;
  folderId: string | null;
  /** Server-derived from the document's first H1; never sent as an input. */
  title: string;
  documentJson: string;
  tradeIds: string[];
  sortOrder: number;
};

export type Folder = {
  id: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
  /** System-owned (the agent-written notes folder). Not renamable or deletable. */
  isSystem: boolean;
};

export function notebookNotes(accountId: string): Promise<Note[]> {
  return invoke("notebook_notes", { accountId });
}

export function notebookFolders(accountId: string): Promise<Folder[]> {
  return invoke("notebook_folders", { accountId });
}

export function createNote(
  accountId: string,
  folderId: string | null,
): Promise<string> {
  // The creator seeds. `jsonToUpdate` is the same builder the server-side projector
  // uses, so a note seeded here and one seeded there are byte-identical.
  const { update, stateVector } = jsonToUpdate(DEFAULT_NOTE_DOC);
  return invoke("create_note", {
    accountId,
    folderId,
    documentJson: DEFAULT_NOTE_DOC,
    seedUpdateB64: toBase64(update),
    seedStateVectorB64: toBase64(stateVector),
  });
}

/// Refreshes the local list preview/title cache. The body itself syncs as CRDT
/// update blobs; this never reaches the server.
export function cacheNoteBody(id: string, documentJson: string): Promise<void> {
  return invoke("cache_note_body", { id, documentJson });
}

/** Base64 Yjs update blobs for a note body, oldest first. Empty = legacy note. */
export function noteUpdates(noteId: string): Promise<string[]> {
  return invoke("note_updates", { noteId });
}

export function appendNoteUpdate(
  noteId: string,
  updateB64: string,
): Promise<void> {
  return invoke("append_note_update", { noteId, updateB64 });
}

/** `folderId: null` moves the note to Uncategorized. */
export function moveNote(
  id: string,
  folderId: string | null,
  sortOrder: number,
): Promise<void> {
  return invoke("move_note", { id, folderId, sortOrder });
}

export function deleteNote(id: string): Promise<void> {
  return invoke("delete_note", { id });
}

export function createFolder(accountId: string, name: string): Promise<string> {
  return invoke("create_folder", { accountId, name });
}

export function renameFolder(id: string, name: string): Promise<void> {
  return invoke("rename_folder", { id, name });
}

export function deleteFolder(id: string): Promise<void> {
  return invoke("delete_folder", { id });
}

// --- Media (content-addressed, local-first; upload/download is Phase F) --

export type MediaResolved = {
  state: "local" | "remote" | "missing";
  fullPath: string | null;
  thumbPath: string | null;
};

export function storeMedia(
  noteId: string,
  accountId: string,
  hash: string,
  mime: string,
  mediaType: string,
  width: number,
  height: number,
  durationSeconds: number,
  originalFilename: string,
  bytes: Uint8Array,
  thumb: Uint8Array,
): Promise<void> {
  return invoke("store_media", {
    noteId,
    accountId,
    hash,
    mime,
    mediaType,
    width,
    height,
    durationSeconds,
    originalFilename,
    bytes,
    thumb,
  });
}

export function resolveMedia(hash: string): Promise<MediaResolved> {
  return invoke("resolve_media", { hash });
}

export function ensureMedia(noteId: string, hash: string): Promise<MediaResolved> {
  return invoke("ensure_media", { noteId, hash });
}

export function deleteMedia(hash: string): Promise<void> {
  return invoke("delete_media", { hash });
}

/** Copies the cached media file into the user's Downloads folder; returns the path. */
export function saveMedia(hash: string, filename: string): Promise<string> {
  return invoke("save_media", { hash, filename });
}
