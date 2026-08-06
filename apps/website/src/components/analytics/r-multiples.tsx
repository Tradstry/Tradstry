"use client";

import { Bar, BarChart, CartesianGrid, Cell, XAxis, YAxis } from "recharts";
import {
  type ChartConfig,
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@/components/ui/chart";
import type { AdvancedAnalytics } from "@/lib/types/analytics";
import { formatNumber, MetricCard, Section } from "./shared";

const WIN_COLOR = "#10b981"; // emerald-500
const LOSS_COLOR = "#f43f5e"; // rose-500

function isLossBucket(label: string) {
  return label.startsWith("<") || label.startsWith("-");
}

const chartConfig = {
  count: { label: "Trades", color: "var(--chart-1)" },
} satisfies ChartConfig;

export function RMultiples({ data }: { data: AdvancedAnalytics }) {
  const slippage =
    data.avgActualR !== null && data.avgPlannedR !== null
      ? data.avgActualR - data.avgPlannedR
      : null;
  const hasDistribution = data.rDistribution.some((b) => b.count > 0);

  return (
    <Section
      title="R-Multiples"
      description="Outcome sizes in risk units, and how execution compared to the plan."
    >
      <div className="grid gap-4 sm:grid-cols-3">
        <MetricCard
          label="Avg Planned R"
          value={
            data.avgPlannedR === null
              ? "—"
              : `${formatNumber(data.avgPlannedR)}R`
          }
          sublabel="Average intended reward-to-risk"
          info={{
            what: "The reward-to-risk you planned at entry, averaged over trades with a target and stop.",
            formula: "mean(planned target ÷ initial risk)",
          }}
        />
        <MetricCard
          label="Avg Actual R"
          value={
            data.avgActualR === null ? "—" : `${formatNumber(data.avgActualR)}R`
          }
          sublabel="Average realized reward-to-risk"
          tone={
            data.avgActualR === null
              ? "neutral"
              : data.avgActualR >= 0
                ? "positive"
                : "negative"
          }
          info={{
            what: "The reward-to-risk you actually realized, averaged over trades with a defined risk.",
            formula: "mean(realized P/L ÷ initial risk)",
          }}
        />
        <MetricCard
          label="R Slippage"
          value={slippage === null ? "—" : `${formatNumber(slippage)}R`}
          sublabel="Actual minus planned R"
          tone={
            slippage === null
              ? "neutral"
              : slippage >= 0
                ? "positive"
                : "negative"
          }
          info={{
            what: "Execution gap: how far realized R landed from the plan. Negative means you left R on the table (cut early, slippage, stops).",
            formula: "avg actual R − avg planned R",
          }}
        />
      </div>

      <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
        <p className="mb-3 text-xs font-medium text-muted-foreground">
          Distribution of outcomes (R buckets)
        </p>
        {hasDistribution ? (
          <ChartContainer config={chartConfig} className="h-[220px] w-full">
            <BarChart data={data.rDistribution} margin={{ left: 4, right: 12 }}>
              <CartesianGrid vertical={false} />
              <XAxis
                dataKey="label"
                tickLine={false}
                axisLine={false}
                tickMargin={8}
              />
              <YAxis
                tickLine={false}
                axisLine={false}
                width={32}
                allowDecimals={false}
              />
              <ChartTooltip
                content={
                  <ChartTooltipContent formatter={(v) => `${v} trades`} />
                }
              />
              <Bar dataKey="count" radius={[4, 4, 0, 0]}>
                {data.rDistribution.map((bucket) => (
                  <Cell
                    key={bucket.label}
                    fill={isLossBucket(bucket.label) ? LOSS_COLOR : WIN_COLOR}
                  />
                ))}
              </Bar>
            </BarChart>
          </ChartContainer>
        ) : (
          <div className="flex h-[220px] items-center justify-center text-xs text-muted-foreground">
            No trades with a defined risk in this range.
          </div>
        )}
      </div>
    </Section>
  );
}
