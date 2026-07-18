"use client";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { AdvancedAnalytics, GroupMetrics } from "@/lib/types/analytics";
import { cn } from "@/lib/utils";
import { DimensionTable } from "./breakdowns";
import {
  formatCurrency,
  formatInt,
  formatPercent,
  formatR,
  formatRatio,
  Section,
} from "./shared";

function MetricRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between py-1.5 text-xs">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium tabular-nums text-foreground">{value}</span>
    </div>
  );
}

function DisciplineStat({
  label,
  value,
  hint,
  tone,
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: "negative";
}) {
  return (
    <div className="rounded-xl border bg-background/90 p-3 shadow-sm">
      <p className="text-[0.62rem] font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </p>
      <p
        className={cn(
          "mt-1 text-lg font-semibold tabular-nums text-foreground",
          tone === "negative" && "text-rose-600",
        )}
      >
        {value}
      </p>
      {hint ? (
        <p className="mt-0.5 text-[0.62rem] text-muted-foreground">{hint}</p>
      ) : null}
    </div>
  );
}

function GroupMetricsCard({
  title,
  caption,
  metrics,
  accent,
}: {
  title: string;
  caption: string;
  metrics: GroupMetrics;
  accent: "positive" | "negative";
}) {
  return (
    <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
      <div className="flex items-baseline justify-between">
        <p className="text-sm font-semibold text-foreground">{title}</p>
        <span
          className={cn(
            "text-xs font-medium",
            accent === "positive" ? "text-emerald-600" : "text-rose-600",
          )}
        >
          {formatInt(metrics.tradeCount)} trades
        </span>
      </div>
      <p className="mt-0.5 text-xs text-muted-foreground">{caption}</p>
      <div className="mt-2 divide-y">
        <MetricRow label="Net P/L" value={formatCurrency(metrics.netProfit)} />
        <MetricRow label="Win Rate" value={formatPercent(metrics.winRate)} />
        <MetricRow
          label="Expectancy"
          value={`${formatCurrency(metrics.expectancyDollars)} · ${formatR(metrics.expectancyR)}`}
        />
        <MetricRow
          label="Profit Factor"
          value={formatRatio(metrics.profitFactor)}
        />
      </div>
    </div>
  );
}

export function Behavioral({ data }: { data: AdvancedAnalytics }) {
  const categories = data.tagBreakdowns.filter((c) => c.tags.length > 0);
  const d = data.discipline;

  return (
    <Section
      title="Behavior & Discipline"
      description="How rule-following and your own tags map to results."
    >
      <div className="grid gap-3 sm:grid-cols-3 xl:grid-cols-6">
        <DisciplineStat
          label="Mistake cost"
          value={formatCurrency(-Math.abs(d.mistakeCost))}
          hint="vs your clean-trade average"
          tone={d.mistakeCost !== 0 ? "negative" : undefined}
        />
        <DisciplineStat
          label="Rule adherence"
          value={
            d.avgRuleAdherence != null
              ? `${d.avgRuleAdherence.toFixed(1)}/5`
              : "—"
          }
          hint="self-scored"
        />
        <DisciplineStat
          label="Avg conviction"
          value={
            d.avgConviction != null ? `${d.avgConviction.toFixed(1)}/5` : "—"
          }
          hint="pre-trade"
        />
        <DisciplineStat
          label="Revenge trades"
          value={formatInt(d.revengeTradeCount)}
        />
        <DisciplineStat
          label="Broke 30-min rule"
          value={formatInt(d.broke30MinCount)}
        />
        <DisciplineStat
          label="Principle violations"
          value={formatInt(d.totalViolations)}
          hint={`across ${formatInt(d.tradesWithViolations)} trades`}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <GroupMetricsCard
          title="Clean"
          caption="Trades with no mistake tags"
          metrics={data.cleanVsFlawed.clean}
          accent="positive"
        />
        <GroupMetricsCard
          title="Flawed"
          caption="Trades tagged with a mistake"
          metrics={data.cleanVsFlawed.flawed}
          accent="negative"
        />
      </div>

      {(data.byConviction.length > 0 || data.byMarketRegime.length > 0) && (
        <div className="grid gap-4 lg:grid-cols-2">
          {data.byConviction.length > 0 ? (
            <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
              <p className="mb-2 text-sm font-semibold text-foreground">
                Conviction → outcome
              </p>
              <DimensionTable rows={data.byConviction} />
            </div>
          ) : null}
          {data.byMarketRegime.length > 0 ? (
            <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
              <p className="mb-2 text-sm font-semibold text-foreground">
                By market regime
              </p>
              <DimensionTable rows={data.byMarketRegime} />
            </div>
          ) : null}
        </div>
      )}

      {categories.length > 0 && (
        <div className="grid gap-4 lg:grid-cols-2">
          {categories.map((category) => (
            <div
              key={category.categoryName}
              className="rounded-2xl border bg-background/90 p-4 shadow-sm"
            >
              <p className="mb-2 text-sm font-semibold text-foreground">
                {category.categoryName}
              </p>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Tag</TableHead>
                    <TableHead className="text-right">Trades</TableHead>
                    <TableHead className="text-right">Win %</TableHead>
                    <TableHead className="text-right">Net P/L</TableHead>
                    <TableHead className="text-right">Expectancy</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {[...category.tags]
                    .sort((a, b) => b.metrics.netProfit - a.metrics.netProfit)
                    .map((tag) => (
                      <TableRow key={tag.key}>
                        <TableCell className="font-medium">{tag.key}</TableCell>
                        <TableCell className="text-right tabular-nums">
                          {formatInt(tag.metrics.tradeCount)}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {formatPercent(tag.metrics.winRate)}
                        </TableCell>
                        <TableCell
                          className={cn(
                            "text-right tabular-nums",
                            tag.metrics.netProfit > 0 && "text-emerald-600",
                            tag.metrics.netProfit < 0 && "text-rose-600",
                          )}
                        >
                          {formatCurrency(tag.metrics.netProfit)}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {formatCurrency(tag.metrics.expectancyDollars)}
                        </TableCell>
                      </TableRow>
                    ))}
                </TableBody>
              </Table>
            </div>
          ))}
        </div>
      )}
    </Section>
  );
}
