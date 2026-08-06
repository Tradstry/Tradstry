"use client";

import {
  Bar,
  BarChart,
  Cell,
  LabelList,
  ReferenceLine,
  XAxis,
  YAxis,
} from "recharts";
import { useActiveWorkspace } from "@/components/workspaces";
import {
  type ChartConfig,
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@/components/ui/chart";
import { Skeleton } from "@/components/ui/skeleton";
import { useAdvancedAnalytics } from "@/hooks/analytics";
import { rangeSublabel } from "@/lib/range-format";
import type { AnalyticsRange, GroupMetrics } from "@/lib/types/analytics";
import { cn, formatPnl } from "@/lib/utils";

const CURRENCY_FMT = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

function formatCurrency(value: number) {
  return formatPnl(value, { precision: "cents" });
}

/** Plain $ formatting (no +/- P&L sign convention) for the mistake-cost
 * hero metric — it's a cost figure, not a realized-P&L delta. */
function formatCost(value: number) {
  return CURRENCY_FMT.format(value);
}

function formatScore(value: number | null) {
  return value === null ? "—" : `${value.toFixed(1)}/5`;
}

function formatPercent(value: number) {
  return `${value.toFixed(1)}%`;
}

function formatFactor(value: number | null) {
  return value === null ? "—" : value.toFixed(2);
}

function StatCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border bg-background/60 px-3 py-2">
      <p className="text-[0.65rem] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
        {label}
      </p>
      <p className="mt-1 text-sm font-semibold tabular-nums text-foreground">
        {value}
      </p>
    </div>
  );
}

const chartConfig = {
  netProfit: { label: "Net P&L" },
} satisfies ChartConfig;

/** Clean vs flawed net P&L on one axis. Same unit, so a diverging bar around zero
 * shows at a glance which bucket actually moved the account. */
function PnlSplitChart({
  clean,
  flawed,
}: {
  clean: GroupMetrics;
  flawed: GroupMetrics;
}) {
  const data = [
    {
      group: "Clean",
      label: `Clean · ${clean.tradeCount}`,
      netProfit: clean.netProfit,
    },
    {
      group: "Flawed",
      label: `Flawed · ${flawed.tradeCount}`,
      netProfit: flawed.netProfit,
    },
  ];

  const max = Math.max(...data.map((d) => Math.abs(d.netProfit)), 1);
  const bound = max * 1.45;

  return (
    <ChartContainer config={chartConfig} className="h-[112px] w-full">
      <BarChart
        accessibilityLayer
        data={data}
        layout="vertical"
        margin={{ left: 0, right: 8, top: 4, bottom: 4 }}
      >
        <XAxis type="number" domain={[-bound, bound]} hide />
        <YAxis
          type="category"
          dataKey="label"
          tickLine={false}
          axisLine={false}
          width={84}
          tick={{ fontSize: 11 }}
        />
        <ReferenceLine x={0} stroke="var(--border)" />
        <ChartTooltip
          cursor={false}
          content={
            <ChartTooltipContent
              hideLabel
              formatter={(value, _name, item) => (
                <div className="flex w-full items-center justify-between gap-4">
                  <span className="text-muted-foreground">
                    {item?.payload?.group} net P&L
                  </span>
                  <span className="font-mono tabular-nums">
                    {formatCurrency(Number(value))}
                  </span>
                </div>
              )}
            />
          }
        />
        <Bar dataKey="netProfit" radius={4} barSize={22}>
          {data.map((d) => (
            <Cell
              key={d.group}
              fill={d.netProfit < 0 ? "var(--loss)" : "var(--profit)"}
            />
          ))}
          <LabelList
            dataKey="netProfit"
            position="right"
            offset={8}
            className="fill-foreground"
            fontSize={11}
            formatter={(v: number) => formatCurrency(v)}
          />
        </Bar>
      </BarChart>
    </ChartContainer>
  );
}

function CompareCell({
  label,
  cleanValue,
  flawedValue,
}: {
  label: string;
  cleanValue: string;
  flawedValue: string;
}) {
  return (
    <div className="rounded-xl border bg-background/60 px-3 py-2">
      <p className="text-[0.65rem] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
        {label}
      </p>
      <dl className="mt-1 space-y-0.5 text-xs">
        <div className="flex items-baseline justify-between gap-2">
          <dt className="text-muted-foreground">Clean</dt>
          <dd className="font-semibold tabular-nums text-emerald-600">
            {cleanValue}
          </dd>
        </div>
        <div className="flex items-baseline justify-between gap-2">
          <dt className="text-muted-foreground">Flawed</dt>
          <dd className="font-semibold tabular-nums text-rose-600">
            {flawedValue}
          </dd>
        </div>
      </dl>
    </div>
  );
}

export function DashboardDisciplineCard({ range }: { range: AnalyticsRange }) {
  const activeWorkspace = useActiveWorkspace();
  const { data, isLoading, isPending, isPlaceholderData, error } =
    useAdvancedAnalytics(activeWorkspace?.id ?? null, { range });

  if (!activeWorkspace) {
    return null;
  }

  if (isLoading || isPending) {
    return (
      <section className="@container/discipline rounded-2xl border bg-background/90 p-4 shadow-sm">
        <div className="space-y-2">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="h-8 w-40" />
        </div>
        <div className="mt-4 grid grid-cols-2 gap-2 @xl/discipline:grid-cols-3 @4xl/discipline:grid-cols-5">
          {["a", "b", "c", "d", "e"].map((key) => (
            <Skeleton key={key} className="h-14 rounded-xl" />
          ))}
        </div>
      </section>
    );
  }

  if (error instanceof Error) {
    return (
      <section className="rounded-2xl border border-rose-200 bg-rose-50 p-4 text-rose-700">
        <p className="text-sm font-semibold uppercase tracking-[0.2em]">
          Discipline Error
        </p>
        <p className="mt-2 text-sm">{error.message}</p>
      </section>
    );
  }

  if (!data) {
    return null;
  }

  const { cleanVsFlawed, discipline } = data;
  const hasTrades = data.tradeCount > 0;
  const hasBehavioralData =
    discipline.avgRuleAdherence !== null ||
    discipline.avgConviction !== null ||
    discipline.revengeTradeCount > 0 ||
    discipline.broke30MinCount > 0 ||
    discipline.tradesWithViolations > 0;
  const mistakeCostIsNegative = discipline.mistakeCost > 0;

  return (
    <section
      className={cn(
        "@container/discipline rounded-2xl border bg-background/90 p-4 shadow-sm transition-opacity duration-200",
        isPlaceholderData && "opacity-60",
      )}
    >
      <div className="flex items-baseline justify-between">
        <p className="text-[0.68rem] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
          Discipline
        </p>
        <span className="text-xs text-muted-foreground">
          {rangeSublabel(range)}
        </span>
      </div>

      {!hasTrades ? (
        <p className="mt-4 text-sm text-muted-foreground">
          No trades in this range yet.
        </p>
      ) : (
        <>
          <div className="mt-3">
            <p
              className={cn(
                "text-2xl font-semibold tabular-nums",
                mistakeCostIsNegative ? "text-rose-600" : "text-emerald-600",
              )}
            >
              {formatCost(discipline.mistakeCost)}
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              Mistake cost · flawed trades vs. clean-trade average
            </p>
          </div>

          <p className="sr-only">
            Clean trades: {cleanVsFlawed.clean.tradeCount}, net{" "}
            {formatCurrency(cleanVsFlawed.clean.netProfit)}. Flawed trades:{" "}
            {cleanVsFlawed.flawed.tradeCount}, net{" "}
            {formatCurrency(cleanVsFlawed.flawed.netProfit)}.
          </p>
          <div className="mt-4" aria-hidden="true">
            <PnlSplitChart
              clean={cleanVsFlawed.clean}
              flawed={cleanVsFlawed.flawed}
            />
          </div>

          <div className="mt-3 grid grid-cols-1 gap-2 @md/discipline:grid-cols-3">
            <CompareCell
              label="Win rate"
              cleanValue={formatPercent(cleanVsFlawed.clean.winRate)}
              flawedValue={formatPercent(cleanVsFlawed.flawed.winRate)}
            />
            <CompareCell
              label="Expectancy"
              cleanValue={formatCurrency(cleanVsFlawed.clean.expectancyDollars)}
              flawedValue={formatCurrency(
                cleanVsFlawed.flawed.expectancyDollars,
              )}
            />
            <CompareCell
              label="Profit factor"
              cleanValue={formatFactor(cleanVsFlawed.clean.profitFactor)}
              flawedValue={formatFactor(cleanVsFlawed.flawed.profitFactor)}
            />
          </div>

          {hasBehavioralData ? (
            <div className="mt-4 grid grid-cols-2 gap-2 @xl/discipline:grid-cols-3 @4xl/discipline:grid-cols-5">
              <StatCell
                label="Rule Adherence"
                value={formatScore(discipline.avgRuleAdherence)}
              />
              <StatCell
                label="Conviction"
                value={formatScore(discipline.avgConviction)}
              />
              <StatCell
                label="Revenge Trades"
                value={String(discipline.revengeTradeCount)}
              />
              <StatCell
                label="30-Min Breaks"
                value={String(discipline.broke30MinCount)}
              />
              <StatCell
                label="Violations"
                value={`${discipline.totalViolations} / ${discipline.tradesWithViolations} trades`}
              />
            </div>
          ) : (
            <p className="mt-3 text-xs text-muted-foreground">
              Log rule adherence, conviction, and mistakes on a trade to unlock
              behavioral stats here.
            </p>
          )}
        </>
      )}
    </section>
  );
}
