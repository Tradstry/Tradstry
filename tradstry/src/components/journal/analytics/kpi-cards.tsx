import { type AdvancedAnalytics } from "../../../backend";
import {
  MetricCard,
  formatCurrency,
  formatPercent,
  formatInt,
  formatR,
  formatRatio,
  formatNumber,
  toneForValue,
} from "./shared";

function sqnRating(sqn: number | null): string {
  if (sqn == null) return "Needs trades with a defined risk";
  if (sqn >= 5) return "Superb";
  if (sqn >= 3) return "Excellent";
  if (sqn >= 2.5) return "Good";
  if (sqn >= 2) return "Average";
  if (sqn >= 1.6) return "Below average";
  return "Poor / hard to trade";
}

export function KpiCards({ data }: { data: AdvancedAnalytics }) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <MetricCard
        label="Net Profit"
        value={formatCurrency(data.netProfit)}
        sublabel="Net realized P/L"
        tone={toneForValue(data.netProfit)}
        info="Σ (exit − entry) × size − fees"
      />
      <MetricCard
        label="Win Rate"
        value={formatPercent(data.winRate)}
        sublabel={`${formatInt(data.tradeCount)} closed trades`}
        tone="neutral"
        info="Winning ÷ (winning + losing) × 100"
      />
      <MetricCard
        label="Expectancy"
        value={formatCurrency(data.expectancyDollars)}
        sublabel={`${formatR(data.expectancyR)} expected per trade`}
        tone={toneForValue(data.expectancyDollars)}
        info="(win% × avg win) − (loss% × avg loss)"
      />
      <MetricCard
        label="Profit Factor"
        value={formatRatio(data.profitFactor)}
        sublabel="Gross profit ÷ gross loss"
        tone="neutral"
        info="Gross profit ÷ gross loss"
      />
      <MetricCard
        label="SQN"
        value={formatNumber(data.sqn)}
        sublabel={sqnRating(data.sqn)}
        tone="neutral"
        info="(mean R ÷ std-dev R) × √N"
      />
      <MetricCard
        label="Trades"
        value={formatInt(data.tradeCount)}
        sublabel={`${formatInt(data.rTradeCount)} with a defined risk`}
        tone="neutral"
      />
      <MetricCard
        label="Average Gain"
        value={formatCurrency(data.averageGain)}
        sublabel={`+${formatPercent(data.averageGainPct)} avg on winners`}
        tone="positive"
      />
      <MetricCard
        label="Average Loss"
        value={formatCurrency(-data.averageLoss)}
        sublabel={`-${formatPercent(data.averageLossPct)} avg on losers`}
        tone="negative"
      />
    </div>
  );
}
