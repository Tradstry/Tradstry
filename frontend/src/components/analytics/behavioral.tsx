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

  return (
    <Section
      title="Behavior & Discipline"
      description="How rule-following and your own tags map to results."
    >
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

      {categories.length > 0 && (
        <div className="space-y-4">
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
