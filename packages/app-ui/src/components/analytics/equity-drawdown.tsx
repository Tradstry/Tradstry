"use client";

import { Area, AreaChart, CartesianGrid, XAxis, YAxis } from "recharts";
import {
  type ChartConfig,
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@tradstry/app-ui/components/ui/chart";
import type { AdvancedAnalytics } from "@tradstry/app-ui/lib/types/analytics";
import {
  formatCurrency,
  formatInt,
  formatNumber,
  formatPercent,
  MetricCard,
  Section,
} from "./shared";

const COMPACT_USD = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  notation: "compact",
  maximumFractionDigits: 1,
});

function shortDate(value: string) {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

const chartConfig = {
  equity: { label: "Equity", color: "var(--chart-1)" },
} satisfies ChartConfig;

export function EquityDrawdown({ data }: { data: AdvancedAnalytics }) {
  return (
    <Section
      title="Equity & Drawdown"
      description="Workspace equity over time and how deep the dips ran."
    >
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Max Drawdown"
          value={formatPercent(data.maxDrawdownPct)}
          sublabel={`${formatCurrency(-Math.abs(data.maxDrawdownDollars))} peak-to-trough`}
          tone="negative"
          info={{
            what: "The largest peak-to-trough drop in equity over the range — your worst losing stretch.",
            formula: "max((peak − equity) ÷ peak)",
          }}
        />
        <MetricCard
          label="Current Drawdown"
          value={formatCurrency(-Math.abs(data.currentDrawdownDollars))}
          sublabel="Distance below the most recent equity peak"
          tone={data.currentDrawdownDollars > 0 ? "negative" : "neutral"}
          info={{
            what: "How far current equity sits below its all-time peak. Zero means you're at a new high.",
            formula: "peak − current equity",
          }}
        />
        <MetricCard
          label="Recovery Factor"
          value={formatNumber(data.recoveryFactor)}
          sublabel="Net profit relative to max drawdown"
          info={{
            what: "How much profit you made per unit of worst drawdown. Higher means smoother growth.",
            formula: "net profit ÷ max drawdown",
          }}
        />
        <MetricCard
          label="Longest Drawdown"
          value={`${formatInt(data.longestDrawdownDays)}d`}
          sublabel="Longest stretch below a prior peak"
          info={{
            what: "The most days equity spent below a previous high before recovering.",
          }}
        />
      </div>

      <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
        {data.equityCurve.length === 0 ? (
          <div className="flex h-[260px] items-center justify-center text-xs text-muted-foreground">
            No equity history in this range.
          </div>
        ) : (
          <ChartContainer config={chartConfig} className="h-[260px] w-full">
            <AreaChart data={data.equityCurve} margin={{ left: 4, right: 12 }}>
              <defs>
                <linearGradient id="fillEquity" x1="0" y1="0" x2="0" y2="1">
                  <stop
                    offset="5%"
                    stopColor="var(--color-equity)"
                    stopOpacity={0.35}
                  />
                  <stop
                    offset="95%"
                    stopColor="var(--color-equity)"
                    stopOpacity={0.03}
                  />
                </linearGradient>
              </defs>
              <CartesianGrid vertical={false} />
              <XAxis
                dataKey="closeDate"
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
                    formatter={(value) => formatCurrency(Number(value))}
                  />
                }
              />
              <Area
                dataKey="equity"
                type="monotone"
                stroke="var(--color-equity)"
                strokeWidth={2}
                fill="url(#fillEquity)"
              />
            </AreaChart>
          </ChartContainer>
        )}
      </div>
    </Section>
  );
}
