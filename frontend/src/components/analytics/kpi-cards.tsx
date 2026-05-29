"use client";

import type { AdvancedAnalytics } from "@/lib/types/analytics";
import {
  formatCurrency,
  formatInt,
  formatNumber,
  formatPercent,
  formatR,
  formatRatio,
  MetricCard,
} from "./shared";

// Van Tharp's rough SQN bands, used only for the sublabel hint.
function sqnRating(value: number | null): string {
  if (value === null) return "Needs trades with a defined risk";
  if (value >= 5) return "Superb";
  if (value >= 3) return "Excellent";
  if (value >= 2.5) return "Good";
  if (value >= 2) return "Average";
  if (value >= 1.6) return "Below average";
  return "Poor / hard to trade";
}

export function AnalyticsKpiCards({ data }: { data: AdvancedAnalytics }) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <MetricCard
        label="Net Profit"
        value={formatCurrency(data.netProfit)}
        sublabel="Net realized P/L across the selected range"
        tone={data.netProfit >= 0 ? "positive" : "negative"}
        info={{
          what: "Net realized profit or loss across all closed trades in the range, after fees.",
          formula: "Σ (exit − entry) × size − fees",
        }}
      />
      <MetricCard
        label="Win Rate"
        value={formatPercent(data.winRate)}
        sublabel={`${formatInt(data.tradeCount)} closed trades`}
        info={{
          what: "Share of closed trades that ended profitable.",
          formula: "winning trades ÷ total closed trades × 100",
        }}
      />
      <MetricCard
        label="Expectancy"
        value={formatCurrency(data.expectancyDollars)}
        sublabel={`${formatR(data.expectancyR)} expected per trade`}
        tone={data.expectancyDollars >= 0 ? "positive" : "negative"}
        info={{
          what: "Average profit you can expect per trade. The R figure is the same in risk multiples.",
          formula: "(win% × avg win) − (loss% × avg loss)",
        }}
      />
      <MetricCard
        label="Profit Factor"
        value={formatRatio(data.profitFactor)}
        sublabel="Gross profit divided by gross loss"
        info={{
          what: "Dollars made per dollar lost. Above 1.0 is profitable; 2.0+ is strong.",
          formula: "gross profit ÷ gross loss",
        }}
      />
      <MetricCard
        label="SQN"
        value={formatNumber(data.sqn)}
        sublabel={sqnRating(data.sqn)}
        info={{
          what: "System Quality Number (Van Tharp): edge size relative to consistency, scaled by sample size. Needs trades with a defined risk.",
          formula: "(mean R ÷ std-dev R) × √N",
        }}
      />
      <MetricCard
        label="Trades"
        value={formatInt(data.tradeCount)}
        sublabel={`${formatInt(data.rTradeCount)} with a defined risk`}
        info={{
          what: "Closed trades in the range. The defined-risk count is how many have an initial risk set — required for R-based metrics (expectancy R, SQN).",
        }}
      />
    </div>
  );
}
