import {
  type AdvancedAnalytics,
  type CategoryBreakdown,
  type GroupMetrics,
} from "../../../backend";
import {
  Section,
  formatCurrency,
  formatInt,
  formatPercent,
  formatR,
  formatRatio,
} from "./shared";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";

function pnlColor(v: number) {
  return v > 0
    ? "text-emerald-600 dark:text-emerald-400"
    : v < 0
      ? "text-red-600 dark:text-red-400"
      : "";
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-sm font-medium tabular-nums">{value}</span>
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
  accent: "clean" | "flawed";
}) {
  return (
    <div className="rounded-2xl border border-zinc-200/80 bg-white/85 p-4 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <div className="flex items-baseline justify-between">
        <h3 className="text-sm font-semibold text-zinc-900 dark:text-zinc-50">
          {title}
        </h3>
        <span
          className={cn(
            "text-sm font-semibold tabular-nums",
            accent === "clean"
              ? "text-emerald-600 dark:text-emerald-400"
              : "text-red-600 dark:text-red-400",
          )}
        >
          {formatInt(metrics.tradeCount)} trades
        </span>
      </div>
      <p className="mt-0.5 text-xs text-muted-foreground">{caption}</p>
      <div className="mt-3 flex flex-col gap-1.5 border-t border-zinc-100 pt-3 dark:border-zinc-800/70">
        <StatRow label="Net P/L" value={formatCurrency(metrics.netProfit)} />
        <StatRow label="Win Rate" value={formatPercent(metrics.winRate)} />
        <StatRow
          label="Expectancy"
          value={`${formatCurrency(metrics.expectancyDollars)} · ${formatR(metrics.expectancyR)}`}
        />
        <StatRow
          label="Profit Factor"
          value={formatRatio(metrics.profitFactor)}
        />
      </div>
    </div>
  );
}

function TagTable({ category }: { category: CategoryBreakdown }) {
  const rows = [...category.tags].sort(
    (a, b) => b.metrics.netProfit - a.metrics.netProfit,
  );
  return (
    <div>
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground">
        {category.categoryName}
      </h3>
      <div className="overflow-hidden rounded-xl border border-zinc-200/80 dark:border-zinc-800">
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
            {rows.map((s) => (
              <TableRow key={s.key}>
                <TableCell className="font-medium">{s.key}</TableCell>
                <TableCell className="text-right tabular-nums">
                  {formatInt(s.metrics.tradeCount)}
                </TableCell>
                <TableCell className="text-right tabular-nums">
                  {formatPercent(s.metrics.winRate)}
                </TableCell>
                <TableCell
                  className={cn(
                    "text-right font-medium tabular-nums",
                    pnlColor(s.metrics.netProfit),
                  )}
                >
                  {formatCurrency(s.metrics.netProfit)}
                </TableCell>
                <TableCell className="text-right tabular-nums">
                  {formatCurrency(s.metrics.expectancyDollars)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

export function Behavioral({ data }: { data: AdvancedAnalytics }) {
  const categories = data.tagBreakdowns.filter((c) => c.tags.length > 0);
  return (
    <Section
      title="Behavior & Discipline"
      description="Do your tagged mistakes cost you, and which tags carry your edge?"
    >
      <div className="grid gap-4 lg:grid-cols-2">
        <GroupMetricsCard
          title="Clean"
          caption="Trades with no mistake tags"
          metrics={data.cleanVsFlawed.clean}
          accent="clean"
        />
        <GroupMetricsCard
          title="Flawed"
          caption="Trades with ≥1 mistake tag"
          metrics={data.cleanVsFlawed.flawed}
          accent="flawed"
        />
      </div>
      {categories.length > 0 && (
        <div className="mt-4 grid gap-4 lg:grid-cols-2">
          {categories.map((c) => (
            <TagTable key={c.categoryName} category={c} />
          ))}
        </div>
      )}
    </Section>
  );
}
