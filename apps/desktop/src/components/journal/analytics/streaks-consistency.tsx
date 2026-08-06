import { type AdvancedAnalytics } from "../../../backend";
import { MetricCard, Section, formatInt, formatNumber, formatDuration } from "./shared";

export function StreaksConsistency({ data }: { data: AdvancedAnalytics }) {
  const streak = data.currentStreak;

  return (
    <Section
      title="Streaks & Consistency"
      description="Win/loss runs, trading pace, and how steady your edge is."
    >
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Current Streak"
          value={streak > 0 ? `${streak} W` : streak < 0 ? `${Math.abs(streak)} L` : "—"}
          sublabel="Consecutive wins or losses, most recent first"
          tone={streak > 0 ? "positive" : streak < 0 ? "negative" : "neutral"}
        />
        <MetricCard
          label="Longest Win Streak"
          value={formatInt(data.longestWinStreak)}
          sublabel="Most consecutive winners"
          tone="positive"
        />
        <MetricCard
          label="Longest Loss Streak"
          value={formatInt(data.longestLossStreak)}
          sublabel="Most consecutive losers"
          tone="negative"
        />
        <MetricCard
          label="Trades / Day"
          value={formatNumber(data.tradesPerDay.avg, 1)}
          sublabel={`max ${formatInt(data.tradesPerDay.max)}, σ ${formatNumber(data.tradesPerDay.stdev, 1)}`}
          tone="neutral"
        />
        <MetricCard
          label="Avg Hold (Winners)"
          value={formatDuration(data.avgHoldWinnersSecs)}
          sublabel="Average time in winning trades"
          tone="neutral"
        />
        <MetricCard
          label="Avg Hold (Losers)"
          value={formatDuration(data.avgHoldLosersSecs)}
          sublabel="Average time in losing trades"
          tone="neutral"
        />
        <MetricCard
          label="Win-Rate Consistency"
          value={data.monthlyWinRateStdev == null ? "—" : `${formatNumber(data.monthlyWinRateStdev, 1)} pp`}
          sublabel="Std-dev of monthly win rate (lower = steadier)"
          tone="neutral"
        />
      </div>
    </Section>
  );
}
