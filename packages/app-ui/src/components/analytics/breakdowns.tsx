"use client";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@tradstry/app-ui/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@tradstry/app-ui/components/ui/tabs";
import type { AdvancedAnalytics, DimensionStat } from "@tradstry/app-ui/lib/types/analytics";
import { cn } from "@tradstry/app-ui/lib/utils";
import {
  formatCurrency,
  formatInt,
  formatPercent,
  formatRatio,
  Section,
} from "./shared";

const TABS: Array<{
  value: string;
  label: string;
  key: keyof AdvancedAnalytics;
}> = [
  { value: "symbol", label: "Symbol", key: "bySymbol" },
  { value: "day", label: "Day", key: "byDayOfWeek" },
  { value: "session", label: "Session", key: "bySession" },
  { value: "holding", label: "Holding", key: "byHolding" },
  { value: "direction", label: "Direction", key: "byDirection" },
  { value: "size", label: "Size", key: "byPositionSize" },
  { value: "playbook", label: "Playbook", key: "byPlaybook" },
];

export function DimensionTable({ rows }: { rows: DimensionStat[] }) {
  if (rows.length === 0) {
    return (
      <div className="flex h-24 items-center justify-center text-xs text-muted-foreground">
        No data for this breakdown in the selected range.
      </div>
    );
  }

  const sorted = [...rows].sort(
    (a, b) => b.metrics.netProfit - a.metrics.netProfit,
  );

  return (
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
        {sorted.map((row) => (
          <TableRow key={row.key}>
            <TableCell className="font-medium">{row.key}</TableCell>
            <TableCell className="text-right tabular-nums">
              {formatInt(row.metrics.tradeCount)}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {formatPercent(row.metrics.winRate)}
            </TableCell>
            <TableCell
              className={cn(
                "text-right tabular-nums",
                row.metrics.netProfit > 0 && "text-emerald-600",
                row.metrics.netProfit < 0 && "text-rose-600",
              )}
            >
              {formatCurrency(row.metrics.netProfit)}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {formatCurrency(row.metrics.expectancyDollars)}
            </TableCell>
            <TableCell className="text-right tabular-nums">
              {formatRatio(row.metrics.profitFactor)}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

export function Breakdowns({ data }: { data: AdvancedAnalytics }) {
  return (
    <Section
      title="Breakdowns"
      description="Performance sliced by symbol, timing, direction, size, and playbook."
    >
      <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
        <Tabs defaultValue="symbol">
          <TabsList className="flex flex-wrap">
            {TABS.map((tab) => (
              <TabsTrigger key={tab.value} value={tab.value}>
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>
          {TABS.map((tab) => (
            <TabsContent key={tab.value} value={tab.value} className="mt-3">
              <DimensionTable rows={data[tab.key] as DimensionStat[]} />
            </TabsContent>
          ))}
        </Tabs>
      </div>
    </Section>
  );
}
