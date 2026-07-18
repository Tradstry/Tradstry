import type { GraphQLFetcher } from "@/lib/client";
import type {
  AdvancedAnalytics,
  AnalyticsTimeFilterInput,
  CalendarAnalytics,
  JournalAnalytics,
} from "@/lib/types/analytics";

const TRADE_OUTCOME_FIELDS = `
  symbol
  symbolName
  amount
`;

const JOURNAL_ANALYTICS_FIELDS = `
  winRate
  cumulativeProfit
  averageRiskToReward
  averageGain
  averageLoss
  averageGainPct
  averageLossPct
  profitFactor
  biggestWin {
    ${TRADE_OUTCOME_FIELDS}
  }
  biggestLoss {
    ${TRADE_OUTCOME_FIELDS}
  }
  rangeStart
  rangeEnd
`;

const CALENDAR_DAY_FIELDS = `
  date
  profit
  tradeCount
  winRate
`;

const CALENDAR_WEEK_FIELDS = `
  weekIndex
  weekStart
  weekEnd
  profit
  tradeCount
  tradingDays
`;

const CALENDAR_ANALYTICS_FIELDS = `
  year
  month
  monthProfit
  tradeCount
  tradingDays
  gridStart
  gridEnd
  days {
    ${CALENDAR_DAY_FIELDS}
  }
  weeks {
    ${CALENDAR_WEEK_FIELDS}
  }
`;

const JOURNAL_ANALYTICS_QUERY = `
  query JournalAnalytics($accountId: String!, $timeFilter: AnalyticsTimeFilterInput!) {
    journalAnalytics(accountId: $accountId, timeFilter: $timeFilter) {
      ${JOURNAL_ANALYTICS_FIELDS}
    }
  }
`;

const CALENDAR_ANALYTICS_QUERY = `
  query CalendarAnalytics($accountId: String!, $year: Int!, $month: Int!) {
    calendarAnalytics(accountId: $accountId, year: $year, month: $month) {
      ${CALENDAR_ANALYTICS_FIELDS}
    }
  }
`;

const GROUP_METRICS_FIELDS = `
  tradeCount
  netProfit
  winRate
  expectancyDollars
  expectancyR
  profitFactor
`;

const DIMENSION_STAT_FIELDS = `
  key
  metrics { ${GROUP_METRICS_FIELDS} }
`;

const ADVANCED_ANALYTICS_FIELDS = `
  tradeCount
  netProfit
  winRate
  expectancyDollars
  expectancyR
  rTradeCount
  profitFactor
  sqn
  averageGain
  averageLoss
  averageGainPct
  averageLossPct
  maxDrawdownDollars
  maxDrawdownPct
  currentDrawdownDollars
  recoveryFactor
  longestDrawdownDays
  equityCurve { closeDate equity }
  startingEquity
  accountEquity
  avgPlannedR
  avgActualR
  rDistribution { label count }
  longestWinStreak
  longestLossStreak
  currentStreak
  avgHoldWinnersSecs
  avgHoldLosersSecs
  monthlyWinRateStdev
  tradesPerDay { avg max stdev }
  bySymbol { ${DIMENSION_STAT_FIELDS} }
  byDayOfWeek { ${DIMENSION_STAT_FIELDS} }
  bySession { ${DIMENSION_STAT_FIELDS} }
  byHolding { ${DIMENSION_STAT_FIELDS} }
  byDirection { ${DIMENSION_STAT_FIELDS} }
  byPositionSize { ${DIMENSION_STAT_FIELDS} }
  byPlaybook { ${DIMENSION_STAT_FIELDS} }
  byConviction { ${DIMENSION_STAT_FIELDS} }
  byMarketRegime { ${DIMENSION_STAT_FIELDS} }
  cleanVsFlawed {
    clean { ${GROUP_METRICS_FIELDS} }
    flawed { ${GROUP_METRICS_FIELDS} }
  }
  discipline {
    flawedTradeCount
    mistakeCost
    avgRuleAdherence
    avgConviction
    revengeTradeCount
    broke30MinCount
    tradesWithViolations
    totalViolations
  }
  tagBreakdowns {
    categoryName
    role
    tags { ${DIMENSION_STAT_FIELDS} }
  }
  rangeStart
  rangeEnd
`;

const ADVANCED_ANALYTICS_QUERY = `
  query AdvancedAnalytics($accountId: String!, $timeFilter: AnalyticsTimeFilterInput!) {
    advancedAnalytics(accountId: $accountId, timeFilter: $timeFilter) {
      ${ADVANCED_ANALYTICS_FIELDS}
    }
  }
`;

export async function fetchAdvancedAnalytics(
  fetcher: GraphQLFetcher,
  accountId: string,
  timeFilter: AnalyticsTimeFilterInput,
): Promise<AdvancedAnalytics> {
  const data = await fetcher<{ advancedAnalytics: AdvancedAnalytics }>(
    ADVANCED_ANALYTICS_QUERY,
    { accountId, timeFilter },
  );
  return data.advancedAnalytics;
}

export async function fetchJournalAnalytics(
  fetcher: GraphQLFetcher,
  accountId: string,
  timeFilter: AnalyticsTimeFilterInput,
): Promise<JournalAnalytics> {
  const data = await fetcher<{ journalAnalytics: JournalAnalytics }>(
    JOURNAL_ANALYTICS_QUERY,
    { accountId, timeFilter },
  );
  return data.journalAnalytics;
}

export async function fetchCalendarAnalytics(
  fetcher: GraphQLFetcher,
  accountId: string,
  year: number,
  month: number,
): Promise<CalendarAnalytics> {
  const data = await fetcher<{ calendarAnalytics: CalendarAnalytics }>(
    CALENDAR_ANALYTICS_QUERY,
    { accountId, year, month },
  );
  return data.calendarAnalytics;
}
