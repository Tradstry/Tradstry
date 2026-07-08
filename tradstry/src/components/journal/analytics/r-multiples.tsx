import { Bar, BarChart, CartesianGrid, Cell, XAxis, YAxis } from "recharts";
import { type AdvancedAnalytics } from "../../../backend";
import {
  MetricCard,
  Section,
  ChartPanel,
  formatNumber,
  type Tone,
} from "./shared";
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";

const WIN_COLOR = "#10b981"; // emerald-500
const LOSS_COLOR = "#f43f5e"; // rose-500

const isLossBucket = (label: string) => /^[<-]/.test(label);

const chartConfig = { count: { label: "Trades" } } satisfies ChartConfig;

export function RMultiples({ data }: { data: AdvancedAnalytics }) {
  const rVal = (n: number | null) => (n == null ? "—" : `${formatNumber(n)}R`);
  const rTone = (n: number | null): Tone =>
    n == null ? "neutral" : n >= 0 ? "positive" : "negative";

  const slip =
    data.avgActualR != null && data.avgPlannedR != null
      ? data.avgActualR - data.avgPlannedR
      : null;

  return (
    <Section
      title="R-Multiples"
      description="Planned vs realized reward-to-risk, and the spread of your outcomes."
    >
      <div className="grid gap-4 sm:grid-cols-3">
        <MetricCard
          label="Avg Planned R"
          value={rVal(data.avgPlannedR)}
          sublabel="Average intended reward-to-risk"
          tone="neutral"
        />
        <MetricCard
          label="Avg Actual R"
          value={rVal(data.avgActualR)}
          sublabel="Average realized reward-to-risk"
          tone={rTone(data.avgActualR)}
        />
        <MetricCard
          label="R Slippage"
          value={rVal(slip)}
          sublabel="Actual minus planned R"
          tone={rTone(slip)}
        />
      </div>

      <ChartPanel title="Distribution of outcomes (R buckets)">
        {data.rDistribution.length === 0 ? (
          <p className="flex h-[220px] items-center justify-center text-sm text-muted-foreground">
            No trades with a defined risk in this range.
          </p>
        ) : (
          <ChartContainer config={chartConfig} className="h-[220px] w-full">
            <BarChart data={data.rDistribution} margin={{ left: 4, right: 8, top: 8, bottom: 0 }}>
              <CartesianGrid vertical={false} />
              <XAxis dataKey="label" tickLine={false} axisLine={false} tickMargin={8} />
              <YAxis tickLine={false} axisLine={false} width={32} allowDecimals={false} />
              <ChartTooltip content={<ChartTooltipContent formatter={(val) => `${val} trades`} />} />
              <Bar dataKey="count" radius={[4, 4, 0, 0]}>
                {data.rDistribution.map((b) => (
                  <Cell key={b.label} fill={isLossBucket(b.label) ? LOSS_COLOR : WIN_COLOR} />
                ))}
              </Bar>
            </BarChart>
          </ChartContainer>
        )}
      </ChartPanel>
    </Section>
  );
}
