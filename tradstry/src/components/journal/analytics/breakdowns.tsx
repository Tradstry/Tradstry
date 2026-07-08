import { type AdvancedAnalytics, type DimensionStat } from "../../../backend";
import {
  Section,
  formatCurrency,
  formatInt,
  formatPercent,
  formatRatio,
} from "./shared";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";

const sortByNet = (rows: DimensionStat[]) =>
  [...rows].sort((a, b) => b.metrics.netProfit - a.metrics.netProfit);

function pnlColor(v: number) {
  return v > 0
    ? "text-emerald-600 dark:text-emerald-400"
    : v < 0
      ? "text-red-600 dark:text-red-400"
      : "";
}

function BreakdownTable({ rows }: { rows: DimensionStat[] }) {
  if (rows.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted-foreground">
        No data in this range.
      </p>
    );
  }
  return (
    <div className="max-h-72 overflow-auto">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Key</TableHead>
            <TableHead className="text-right">Trades</TableHead>
            <TableHead className="text-right">Win %</TableHead>
            <TableHead className="text-right">Net P/L</TableHead>
            <TableHead className="text-right">Expectancy</TableHead>
            <TableHead className="text-right">PF</TableHead>
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
              <TableCell className="text-right tabular-nums">
                {formatRatio(s.metrics.profitFactor)}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

const TABS: { value: string; label: string; key: keyof AdvancedAnalytics }[] = [
  { value: "symbol", label: "Symbol", key: "bySymbol" },
  { value: "weekday", label: "Weekday", key: "byDayOfWeek" },
  { value: "session", label: "Session", key: "bySession" },
  { value: "holding", label: "Holding", key: "byHolding" },
  { value: "direction", label: "Direction", key: "byDirection" },
  { value: "size", label: "Size", key: "byPositionSize" },
  { value: "playbook", label: "Playbook", key: "byPlaybook" },
];

export function Breakdowns({ data }: { data: AdvancedAnalytics }) {
  return (
    <Section
      title="Breakdowns"
      description="Where your P/L actually comes from, sliced seven ways."
    >
      <div className="rounded-2xl border border-zinc-200/80 bg-white/85 p-2 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
        <Tabs defaultValue="symbol">
          <TabsList className="flex-wrap">
            {TABS.map((t) => (
              <TabsTrigger key={t.value} value={t.value}>
                {t.label}
              </TabsTrigger>
            ))}
          </TabsList>
          {TABS.map((t) => (
            <TabsContent key={t.value} value={t.value}>
              <BreakdownTable rows={sortByNet(data[t.key] as DimensionStat[])} />
            </TabsContent>
          ))}
        </Tabs>
      </div>
    </Section>
  );
}
