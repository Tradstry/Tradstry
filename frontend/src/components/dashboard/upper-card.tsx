"use client";

import { useActiveAccount } from "@/components/accounts";
import { Skeleton } from "@/components/ui/skeleton";
import { useJournalAnalytics } from "@/hooks/analytics";
import { rangeSublabel } from "@/lib/range-format";
import type { AnalyticsRange } from "@/lib/types/analytics";
import { cn, formatPnl } from "@/lib/utils";

const percentFormatter = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

function formatCurrency(value: number) {
  return formatPnl(value, { precision: "cents" });
}

function formatPercent(value: number) {
  return `${percentFormatter.format(value)}%`;
}

function formatRatio(value: number | null) {
  if (value === null) return "-";
  return `${value.toFixed(2)}x`;
}

function MetricCard({
  label,
  value,
  sublabel,
  tone = "neutral",
}: {
  label: string;
  value: string;
  sublabel: string;
  tone?: "neutral" | "positive" | "negative";
}) {
  return (
    <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
      <p className="text-[0.68rem] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
        {label}
      </p>
      <p
        className={cn(
          "mt-3 text-2xl font-semibold",
          tone === "positive" && "text-emerald-600",
          tone === "negative" && "text-rose-600",
        )}
      >
        {value}
      </p>
      <p className="mt-1 text-xs text-muted-foreground">{sublabel}</p>
    </div>
  );
}

export function DashboardUpperCard({ range }: { range: AnalyticsRange }) {
  const activeAccount = useActiveAccount();
  const { data, isLoading, isPending, isPlaceholderData, error } =
    useJournalAnalytics(activeAccount?.id ?? null, { range });
  // This is intentionally local, rather than Countly Remote Config (a paid
  // feature). It is evaluated at build time; unset means show all metrics.
  const hideSecondaryMetrics =
    process.env.NEXT_PUBLIC_DASHBOARD_COMPACT_METRICS === "true";

  if (!activeAccount) {
    return (
      <section className="rounded-2xl border bg-background/80 p-6">
        <p className="text-sm font-medium text-foreground">No active account</p>
        <p className="mt-2 text-sm text-muted-foreground">
          Select an account to load dashboard analytics.
        </p>
      </section>
    );
  }

  if (isLoading || isPending) {
    return (
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <div className="space-y-2">
            <Skeleton className="h-5 w-36" />
            <Skeleton className="h-4 w-56" />
          </div>
          <Skeleton className="h-10 w-24 rounded-xl" />
        </div>
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {["a", "b", "c", "d"].map((key) => (
            <Skeleton key={key} className="h-32 rounded-2xl" />
          ))}
        </div>
      </section>
    );
  }

  if (error instanceof Error) {
    return (
      <section className="rounded-2xl border border-rose-200 bg-rose-50 p-6 text-rose-700">
        <p className="text-sm font-semibold uppercase tracking-[0.2em]">
          Analytics Error
        </p>
        <p className="mt-2 text-sm">{error.message}</p>
      </section>
    );
  }

  if (!data) {
    return null;
  }

  return (
    <section
      className={cn(
        "space-y-4 pt-3 transition-opacity duration-200",
        isPlaceholderData && "opacity-60",
      )}
    >
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Cumulative Profit"
          value={formatCurrency(data.cumulativeProfit)}
          sublabel={`Net realized P/L · ${rangeSublabel(range)}`}
          tone={data.cumulativeProfit >= 0 ? "positive" : "negative"}
        />
        <MetricCard
          label="Win Rate"
          value={formatPercent(data.winRate)}
          sublabel="Winning trades divided by total closed trades"
        />
        <MetricCard
          label="Average R:R"
          value={`${data.averageRiskToReward.toFixed(2)}R`}
          sublabel="Average realized reward-to-risk"
        />
        <MetricCard
          label="Profit Factor"
          value={formatRatio(data.profitFactor)}
          sublabel="Gross profit divided by gross loss"
        />
      </div>

      <div
        className={cn(
          "grid gap-4 md:grid-cols-2 xl:grid-cols-4",
          hideSecondaryMetrics && "hidden",
        )}
      >
        <MetricCard
          label="Average Gain"
          value={formatCurrency(data.averageGain)}
          sublabel={`+${formatPercent(data.averageGainPct)} avg on winning trades`}
          tone="positive"
        />
        <MetricCard
          label="Average Loss"
          value={formatCurrency(-data.averageLoss)}
          sublabel={`-${formatPercent(data.averageLossPct)} avg on losing trades`}
          tone="negative"
        />
        <MetricCard
          label="Biggest Win"
          value={data.biggestWin?.symbol ?? "-"}
          sublabel={
            data.biggestWin
              ? formatCurrency(data.biggestWin.amount)
              : "No winning trade in this range"
          }
          tone={data.biggestWin ? "positive" : "neutral"}
        />
        <MetricCard
          label="Biggest Loss"
          value={data.biggestLoss?.symbol ?? "-"}
          sublabel={
            data.biggestLoss
              ? formatCurrency(data.biggestLoss.amount)
              : "No losing trade in this range"
          }
          tone={data.biggestLoss ? "negative" : "neutral"}
        />
      </div>
    </section>
  );
}
