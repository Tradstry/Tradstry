import { Area, AreaChart, CartesianGrid, XAxis, YAxis } from "recharts";
import { type AdvancedAnalytics } from "../../../backend";
import {
  MetricCard,
  Section,
  ChartPanel,
  formatCurrency,
  formatInt,
  formatNumber,
  formatPercent,
} from "./shared";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";

const chartConfig = {
  equity: { label: "Equity", color: "var(--chart-1)" },
} satisfies ChartConfig;

const shortDate = (v: string) =>
  new Date(v).toLocaleDateString("en-US", { month: "short", day: "numeric" });

const compactUsd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  notation: "compact",
  maximumFractionDigits: 1,
});

export function EquityDrawdown({ data }: { data: AdvancedAnalytics }) {
  return (
    <Section
      title="Equity & Drawdown"
      description="Cumulative P/L over the period and how far it fell from its peaks."
    >
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Max Drawdown"
          value={formatPercent(data.maxDrawdownPct)}
          sublabel={`${formatCurrency(-Math.abs(data.maxDrawdownDollars))} peak-to-trough`}
          tone="negative"
        />
        <MetricCard
          label="Current Drawdown"
          value={formatCurrency(-Math.abs(data.currentDrawdownDollars))}
          sublabel="Distance below the most recent equity peak"
          tone={data.currentDrawdownDollars > 0 ? "negative" : "neutral"}
        />
        <MetricCard
          label="Recovery Factor"
          value={formatNumber(data.recoveryFactor)}
          sublabel="Net profit relative to max drawdown"
          tone="neutral"
        />
        <MetricCard
          label="Longest Drawdown"
          value={`${formatInt(data.longestDrawdownDays)}d`}
          sublabel="Longest stretch below a prior peak"
          tone="neutral"
        />
      </div>

      <ChartPanel title="Equity curve">
        {data.equityCurve.length === 0 ? (
          <p className="flex h-[260px] items-center justify-center text-sm text-muted-foreground">
            No equity history in this range.
          </p>
        ) : (
          <ChartContainer config={chartConfig} className="h-[260px] w-full">
            <AreaChart
              data={data.equityCurve}
              margin={{ left: 4, right: 8, top: 8, bottom: 0 }}
            >
              <defs>
                <linearGradient id="fillEquity" x1="0" y1="0" x2="0" y2="1">
                  <stop
                    offset="5%"
                    stopColor="var(--color-equity)"
                    stopOpacity={0.3}
                  />
                  <stop
                    offset="95%"
                    stopColor="var(--color-equity)"
                    stopOpacity={0}
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
                tickFormatter={(v) => compactUsd.format(Number(v))}
              />
              <ChartTooltip
                content={
                  <ChartTooltipContent
                    labelFormatter={(l) => shortDate(String(l))}
                    formatter={(val) => formatCurrency(Number(val))}
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
      </ChartPanel>
    </Section>
  );
}
