"use client";

import { CartesianGrid, Line, LineChart, XAxis, YAxis } from "recharts";
import { useActiveAccount } from "@/components/accounts";
import { Button } from "@/components/ui/button";
import {
  type ChartConfig,
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
} from "@/components/ui/chart";
import { Skeleton } from "@/components/ui/skeleton";
import {
  useAccountEquityHistory,
  useRebuildAccountEquityHistory,
} from "@/hooks/equity";
import { rangeSublabel } from "@/lib/range-format";
import type { AnalyticsRange } from "@/lib/types/analytics";
import type { EquityHistoryHealth } from "@/lib/types/equity";
import { cn } from "@/lib/utils";

const USD = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 2,
});

const COMPACT_USD = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  notation: "compact",
  maximumFractionDigits: 1,
});

const DRIFT_TOLERANCE = 0.01;

function shortDate(value: string) {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

/** Without funding transactions cash starts at zero, so the curve is cumulative P&L
 * rather than account value, and the funding-adjusted line duplicates it exactly. */
const PNL_CONFIG = {
  equity: { label: "Cumulative P&L", color: "var(--chart-1)" },
} satisfies ChartConfig;

const ACCOUNT_VALUE_CONFIG = {
  equity: { label: "Account value", color: "var(--chart-1)" },
  fundingAdjustedEquity: {
    label: "Performance (excl. deposits)",
    color: "var(--chart-2)",
  },
} satisfies ChartConfig;

function trustWarning(health: EquityHistoryHealth | null): string | null {
  if (!health) return null;

  const reasons: string[] = [];
  const { drift, reportedEquity, unclassifiedTypes, missingPriceDays } = health;

  if (
    drift !== null &&
    reportedEquity !== null &&
    Math.abs(reportedEquity) > 0 &&
    Math.abs(drift / reportedEquity) > DRIFT_TOLERANCE
  ) {
    reasons.push(
      `the reconstructed value is off by ${USD.format(drift)} from what your broker reports`,
    );
  }
  if (unclassifiedTypes.length > 0) {
    reasons.push(
      `${unclassifiedTypes.length} transaction type${unclassifiedTypes.length === 1 ? "" : "s"} we don't recognize (${unclassifiedTypes.slice(0, 3).join(", ")})`,
    );
  }
  if (missingPriceDays > 0) {
    reasons.push(`${missingPriceDays} days with no price data`);
  }

  return reasons.length > 0
    ? `This curve may be inaccurate: ${reasons.join("; ")}.`
    : null;
}

export function DashboardEquityHistoryCard({
  range,
}: {
  range: AnalyticsRange;
}) {
  const activeAccount = useActiveAccount();
  const { data, isLoading, isPending, isPlaceholderData, error } =
    useAccountEquityHistory(activeAccount?.id ?? null, range);
  const rebuild = useRebuildAccountEquityHistory();

  if (!activeAccount) {
    return null;
  }

  if (isLoading || isPending) {
    return (
      <section className="rounded-2xl border bg-background/90 p-4 shadow-sm">
        <Skeleton className="h-4 w-32" />
        <Skeleton className="mt-3 h-[260px] w-full rounded-xl" />
      </section>
    );
  }

  if (error instanceof Error) {
    return (
      <section className="rounded-2xl border border-rose-200 bg-rose-50 p-4 text-rose-700">
        <p className="text-sm font-semibold uppercase tracking-[0.2em]">
          Equity History Error
        </p>
        <p className="mt-2 text-sm">{error.message}</p>
      </section>
    );
  }

  if (!data) {
    return null;
  }

  const warning = trustWarning(data.health);
  const latest = data.points.at(-1);
  const hasFunding = data.points.some((p) => p.netContributions !== 0);
  const chartConfig = hasFunding ? ACCOUNT_VALUE_CONFIG : PNL_CONFIG;

  return (
    <section
      className={cn(
        "rounded-2xl border bg-background/90 p-4 shadow-sm transition-opacity duration-200",
        isPlaceholderData && "opacity-60",
      )}
    >
      <div className="flex items-baseline justify-between">
        <p className="text-[0.68rem] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
          {hasFunding ? "Account Value" : "Performance"}
        </p>
        <div className="flex items-center gap-3">
          <span className="text-xs text-muted-foreground">
            {rangeSublabel(range)}
          </span>
          {data.points.length > 0 ? (
            <Button
              size="sm"
              variant="ghost"
              className="h-7 px-2 text-xs"
              disabled={rebuild.isPending}
              onClick={() => rebuild.mutate(activeAccount.id)}
            >
              {rebuild.isPending ? "Rebuilding…" : "Rebuild"}
            </Button>
          ) : null}
        </div>
      </div>

      {data.points.length === 0 ? (
        <div className="mt-4 flex flex-col items-start gap-3">
          <p className="text-sm text-muted-foreground">
            No history yet. It is reconstructed from your broker transactions —
            rebuild it to see how the account has performed.
          </p>
          <Button
            size="sm"
            variant="outline"
            disabled={rebuild.isPending}
            onClick={() => rebuild.mutate(activeAccount.id)}
          >
            {rebuild.isPending ? "Rebuilding…" : "Rebuild history"}
          </Button>
          {rebuild.error instanceof Error ? (
            <p className="text-xs text-rose-600">{rebuild.error.message}</p>
          ) : null}
        </div>
      ) : (
        <>
          {latest ? (
            <div className="mt-3 flex items-baseline gap-4">
              <p
                className={cn(
                  "text-2xl font-semibold tabular-nums",
                  !hasFunding &&
                    (latest.equity < 0 ? "text-rose-600" : "text-emerald-600"),
                )}
              >
                {USD.format(latest.equity)}
              </p>
              <p className="text-xs text-muted-foreground">
                {hasFunding
                  ? `${USD.format(latest.fundingAdjustedEquity)} excluding deposits`
                  : "Cumulative P&L · cash + open positions since your first trade"}
              </p>
            </div>
          ) : null}

          {warning ? (
            <div className="mt-3 rounded-xl border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-800">
              {warning}
            </div>
          ) : null}

          <ChartContainer
            config={chartConfig}
            className="mt-4 h-[260px] w-full"
          >
            <LineChart data={data.points} margin={{ left: 4, right: 12 }}>
              <CartesianGrid vertical={false} />
              <XAxis
                dataKey="date"
                tickLine={false}
                axisLine={false}
                tickMargin={8}
                minTickGap={32}
                tickFormatter={shortDate}
              />
              <YAxis
                tickLine={false}
                axisLine={false}
                width={56}
                tickFormatter={(v: number) => COMPACT_USD.format(v)}
              />
              <ChartTooltip
                content={
                  <ChartTooltipContent
                    labelFormatter={(label) => shortDate(String(label))}
                    formatter={(value, name) => (
                      <div className="flex w-full items-center justify-between gap-4">
                        <span className="text-muted-foreground">
                          {chartConfig[name as keyof typeof chartConfig]
                            ?.label ?? name}
                        </span>
                        <span className="font-mono tabular-nums">
                          {USD.format(Number(value))}
                        </span>
                      </div>
                    )}
                  />
                }
              />
              {hasFunding ? (
                <ChartLegend content={<ChartLegendContent />} />
              ) : null}
              <Line
                dataKey="equity"
                type="monotone"
                stroke="var(--color-equity)"
                strokeWidth={2}
                dot={false}
              />
              {hasFunding ? (
                <Line
                  dataKey="fundingAdjustedEquity"
                  type="monotone"
                  stroke="var(--color-fundingAdjustedEquity)"
                  strokeWidth={2}
                  strokeDasharray="4 4"
                  dot={false}
                />
              ) : null}
            </LineChart>
          </ChartContainer>
        </>
      )}
    </section>
  );
}
