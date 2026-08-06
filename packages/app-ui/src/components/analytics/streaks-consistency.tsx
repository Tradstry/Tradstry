"use client";

import type { AdvancedAnalytics } from "@tradstry/app-ui/lib/types/analytics";
import {
  formatDuration,
  formatInt,
  formatNumber,
  MetricCard,
  Section,
} from "./shared";

function streakValue(n: number) {
  if (n > 0) return `${n} W`;
  if (n < 0) return `${Math.abs(n)} L`;
  return "—";
}

export function StreaksConsistency({ data }: { data: AdvancedAnalytics }) {
  return (
    <Section
      title="Streaks & Consistency"
      description="Run lengths, hold times, and how steady your results are."
    >
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Current Streak"
          value={streakValue(data.currentStreak)}
          sublabel="Consecutive wins or losses, most recent first"
          tone={
            data.currentStreak > 0
              ? "positive"
              : data.currentStreak < 0
                ? "negative"
                : "neutral"
          }
          info={{
            what: "How many trades in a row ended the same way through your latest trade. W = winners, L = losers.",
          }}
        />
        <MetricCard
          label="Longest Win Streak"
          value={formatInt(data.longestWinStreak)}
          sublabel="Most consecutive winners"
          tone="positive"
          info={{
            what: "The longest run of back-to-back winning trades in the range.",
          }}
        />
        <MetricCard
          label="Longest Loss Streak"
          value={formatInt(data.longestLossStreak)}
          sublabel="Most consecutive losers"
          tone="negative"
          info={{
            what: "The longest run of back-to-back losing trades in the range.",
          }}
        />
        <MetricCard
          label="Trades / Day"
          value={formatNumber(data.tradesPerDay.avg, 1)}
          sublabel={`max ${formatInt(data.tradesPerDay.max)}, σ ${formatNumber(data.tradesPerDay.stdev, 1)}`}
          info={{
            what: "Average closed trades per active trading day, with the busiest day and the day-to-day variability (σ).",
          }}
        />
        <MetricCard
          label="Avg Hold (Winners)"
          value={formatDuration(data.avgHoldWinnersSecs)}
          sublabel="Average time in winning trades"
          info={{
            what: "How long, on average, you held trades that ended in profit.",
          }}
        />
        <MetricCard
          label="Avg Hold (Losers)"
          value={formatDuration(data.avgHoldLosersSecs)}
          sublabel="Average time in losing trades"
          info={{
            what: "How long, on average, you held trades that ended in a loss. Holding losers much longer than winners is a common leak.",
          }}
        />
        <MetricCard
          label="Win-Rate Consistency"
          value={
            data.monthlyWinRateStdev === null
              ? "—"
              : `${formatNumber(data.monthlyWinRateStdev, 1)} pp`
          }
          sublabel="Std-dev of monthly win rate (lower = steadier)"
          info={{
            what: "How much your win rate swings month to month, in percentage points. Lower means more consistent results.",
            formula: "std-dev(monthly win rate)",
          }}
        />
      </div>
    </Section>
  );
}
