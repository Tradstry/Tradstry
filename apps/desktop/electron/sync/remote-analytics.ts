import type { GraphqlClient } from "./protocol.ts";
import type { AnalyticsCommands } from "./service.ts";

const GROUP = "tradeCount netProfit winRate expectancyDollars expectancyR profitFactor";
const DIMENSION = `key metrics { ${GROUP} }`;

const JOURNAL = `query JournalAnalytics($workspaceId: String!, $timeFilter: AnalyticsTimeFilterInput!) {
  journalAnalytics(workspaceId: $workspaceId, timeFilter: $timeFilter) {
    winRate cumulativeProfit averageRiskToReward averageGain averageLoss averageGainPct averageLossPct
    profitFactor biggestWin { symbol symbolName amount } biggestLoss { symbol symbolName amount }
    rangeStart rangeEnd
  }
}`;
const CALENDAR = `query CalendarAnalytics($workspaceId: String!, $year: Int!, $month: Int!) {
  calendarAnalytics(workspaceId: $workspaceId, year: $year, month: $month) {
    year month monthProfit tradeCount tradingDays gridStart gridEnd
    days { date profit tradeCount winRate }
    weeks { weekIndex weekStart weekEnd profit tradeCount tradingDays }
  }
}`;
const ADVANCED = `query AdvancedAnalytics($workspaceId: String!, $timeFilter: AnalyticsTimeFilterInput!) {
  advancedAnalytics(workspaceId: $workspaceId, timeFilter: $timeFilter) {
    tradeCount netProfit winRate expectancyDollars expectancyR rTradeCount profitFactor sqn
    averageGain averageLoss averageGainPct averageLossPct maxDrawdownDollars maxDrawdownPct
    currentDrawdownDollars recoveryFactor longestDrawdownDays equityCurve { closeDate equity }
    startingEquity accountEquity avgPlannedR avgActualR rDistribution { label count }
    longestWinStreak longestLossStreak currentStreak avgHoldWinnersSecs avgHoldLosersSecs
    monthlyWinRateStdev tradesPerDay { avg max stdev }
    bySymbol { ${DIMENSION} } byDayOfWeek { ${DIMENSION} } bySession { ${DIMENSION} }
    byHolding { ${DIMENSION} } byDirection { ${DIMENSION} } byPositionSize { ${DIMENSION} }
    byPlaybook { ${DIMENSION} } byConviction { ${DIMENSION} } byMarketRegime { ${DIMENSION} }
    cleanVsFlawed { clean { ${GROUP} } flawed { ${GROUP} } }
    discipline { flawedTradeCount mistakeCost avgRuleAdherence avgConviction revengeTradeCount broke30MinCount tradesWithViolations totalViolations }
    tagBreakdowns { categoryName role tags { ${DIMENSION} } }
    rangeStart rangeEnd
  }
}`;

export class RemoteAnalytics implements AnalyticsCommands {
  readonly #graphql: GraphqlClient;
  constructor(graphql: GraphqlClient) {
    this.#graphql = graphql;
  }
  async journal(accountId: string, timeFilter: unknown): Promise<unknown> {
    const data = (await this.#graphql(JOURNAL, { workspaceId: accountId, timeFilter })) as { journalAnalytics?: unknown };
    return required(data.journalAnalytics, "journalAnalytics");
  }
  async calendar(accountId: string, year: number, month: number): Promise<unknown> {
    const data = (await this.#graphql(CALENDAR, { workspaceId: accountId, year, month })) as { calendarAnalytics?: unknown };
    return required(data.calendarAnalytics, "calendarAnalytics");
  }
  async advanced(accountId: string, timeFilter: unknown): Promise<unknown> {
    const data = (await this.#graphql(ADVANCED, { workspaceId: accountId, timeFilter })) as { advancedAnalytics?: unknown };
    return required(data.advancedAnalytics, "advancedAnalytics");
  }
}

function required<T>(value: T | null | undefined, name: string): T {
  if (value === null || value === undefined) throw new Error(`missing ${name} in response`);
  return value;
}
