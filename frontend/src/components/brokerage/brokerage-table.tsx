"use client";

import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  type SortingState,
  useReactTable,
} from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { RANGE_PRESETS } from "@/lib/range-presets";
import type { BrokerageTransaction } from "@/lib/types/brokerage";
import type { AnalyticsRange } from "@/lib/types/analytics";

// ---------------------------------------------------------------------------
// Type badge colors
// ---------------------------------------------------------------------------

const TYPE_COLORS: Record<string, string> = {
  BUY: "border-emerald-200 dark:border-emerald-900 bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300",
  SELL: "border-rose-200 dark:border-rose-900 bg-rose-50 dark:bg-rose-950/50 text-rose-700 dark:text-rose-300",
  DIVIDEND: "border-indigo-200 dark:border-indigo-900 bg-indigo-50 dark:bg-indigo-950/50 text-indigo-700 dark:text-indigo-300",
  STOCK_DIVIDEND: "border-indigo-200 dark:border-indigo-900 bg-indigo-50 dark:bg-indigo-950/50 text-indigo-700 dark:text-indigo-300",
  INTEREST: "border-indigo-200 dark:border-indigo-900 bg-indigo-50 dark:bg-indigo-950/50 text-indigo-700 dark:text-indigo-300",
  REI: "border-indigo-200 dark:border-indigo-900 bg-indigo-50 dark:bg-indigo-950/50 text-indigo-700 dark:text-indigo-300",
  OPTIONEXPIRATION: "border-violet-200 dark:border-violet-900 bg-violet-50 dark:bg-violet-950/50 text-violet-700 dark:text-violet-300",
  OPTIONASSIGNMENT: "border-violet-200 dark:border-violet-900 bg-violet-50 dark:bg-violet-950/50 text-violet-700 dark:text-violet-300",
  OPTIONEXERCISE: "border-violet-200 dark:border-violet-900 bg-violet-50 dark:bg-violet-950/50 text-violet-700 dark:text-violet-300",
  TRANSFER: "border-amber-200 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300",
  CONTRIBUTION: "border-amber-200 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300",
  WITHDRAWAL: "border-amber-200 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300",
  EXTERNAL_ASSET_TRANSFER_IN: "border-amber-200 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300",
  EXTERNAL_ASSET_TRANSFER_OUT: "border-amber-200 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300",
};
const DEFAULT_TYPE_COLOR = "border-border bg-muted text-muted-foreground";

function TypeBadge({ type }: { type: string }) {
  const cls = TYPE_COLORS[type] ?? DEFAULT_TYPE_COLOR;
  return (
    <span
      className={`inline-flex rounded-full border px-2.5 py-0.5 text-[0.65rem] font-medium ${cls}`}
    >
      {type}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

function fmtDate(iso: string | null): string {
  if (!iso) return "\u2014";
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
  }).format(new Date(iso));
}

function fmtCurrency(value: number | null, currency = "USD"): string {
  if (value == null) return "\u2014";
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
  }).format(value);
}

function amountClasses(value: number | null): string {
  if (value == null) return "text-muted-foreground";
  return value < 0
    ? "text-rose-600 dark:text-rose-400 font-medium"
    : "text-emerald-600 dark:text-emerald-400 font-medium";
}

function groupNetAmount(txs: BrokerageTransaction[]): number {
  return txs.reduce((sum, tx) => sum + (tx.amount ?? 0), 0);
}

// ---------------------------------------------------------------------------
// Columns
// ---------------------------------------------------------------------------

function buildColumns(
  linkedIds: Set<string>,
): ColumnDef<BrokerageTransaction>[] {
  return [
    {
      id: "select",
      header: ({ table }) => (
        <input
          type="checkbox"
          className="size-3.5 rounded border-muted-foreground/30"
          checked={table.getIsAllPageRowsSelected()}
          onChange={(e) => table.toggleAllPageRowsSelected(e.target.checked)}
        />
      ),
      cell: ({ row }) => {
        const isLinked = linkedIds.has(row.original.id);
        return (
          <div className="flex items-center gap-1.5">
            <input
              type="checkbox"
              className="size-3.5 rounded border-muted-foreground/30"
              checked={row.getIsSelected()}
              disabled={isLinked}
              onChange={(e) => row.toggleSelected(e.target.checked)}
            />
            {isLinked && (
              <span className="inline-flex rounded-full border border-indigo-200 dark:border-indigo-900 bg-indigo-50 dark:bg-indigo-950/50 px-1.5 py-0 text-[0.55rem] font-medium text-indigo-600 dark:text-indigo-300">
                Journaled
              </span>
            )}
          </div>
        );
      },
      enableSorting: false,
    },
    {
      accessorKey: "tradeDate",
      header: "Date",
      cell: ({ row }) => (
        <span className="text-muted-foreground">
          {fmtDate(row.original.tradeDate)}
        </span>
      ),
    },
    {
      accessorKey: "symbol",
      header: "Symbol",
      cell: ({ row }) => {
        const { symbol, symbolDescription } = row.original;
        if (!symbol)
          return <span className="text-muted-foreground">{"\u2014"}</span>;
        return (
          <div className="flex flex-col">
            <span className="font-medium">{symbol}</span>
            {symbolDescription && (
              <span className="max-w-[12rem] truncate text-[0.65rem] text-muted-foreground">
                {symbolDescription}
              </span>
            )}
          </div>
        );
      },
    },
    {
      accessorKey: "transactionType",
      header: "Type",
      cell: ({ row }) => <TypeBadge type={row.original.transactionType} />,
    },
    {
      accessorKey: "units",
      header: "Qty",
      cell: ({ row }) => {
        const v = row.original.units;
        return v !== 0 ? Math.abs(v).toLocaleString() : "\u2014";
      },
    },
    {
      accessorKey: "price",
      header: "Price",
      cell: ({ row }) => fmtCurrency(row.original.price, row.original.currency),
    },
    {
      accessorKey: "amount",
      header: "Amount",
      cell: ({ row }) => (
        <span className={amountClasses(row.original.amount)}>
          {fmtCurrency(row.original.amount, row.original.currency)}
        </span>
      ),
    },
    {
      accessorKey: "fee",
      header: "Fee",
      cell: ({ row }) => (
        <span className={row.original.fee === 0 ? "text-muted-foreground" : ""}>
          {fmtCurrency(row.original.fee, row.original.currency)}
        </span>
      ),
    },
  ];
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface BrokerageTableProps {
  transactions: BrokerageTransaction[];
  total: number;
  page: number;
  pageSize: number;
  hasNextPage: boolean;
  hasPrevPage: boolean;
  onNextPage: () => void;
  onPrevPage: () => void;
  onPageSizeChange: (size: number) => void;
  isLoading: boolean;
  linkedTransactionIds?: Set<string>;
  selectedIds: Set<string>;
  onSelectedIdsChange: (ids: Set<string>) => void;
  dateRange?: AnalyticsRange;
  onDateRangeChange?: (range: AnalyticsRange) => void;
  symbolSearch?: string;
  onSymbolSearchChange?: (v: string) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function BrokerageTable({
  transactions,
  total,
  page,
  pageSize,
  hasNextPage,
  hasPrevPage,
  onNextPage,
  onPrevPage,
  onPageSizeChange,
  isLoading,
  linkedTransactionIds = new Set(),
  selectedIds,
  onSelectedIdsChange,
  dateRange = "ALL",
  onDateRangeChange,
  symbolSearch = "",
  onSymbolSearchChange,
}: BrokerageTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);

  const columns = buildColumns(linkedTransactionIds);

  // Group transactions by month, then by symbol within each month
  const monthGroups = useMemo(() => {
    const map = new Map<string, Map<string, BrokerageTransaction[]>>();
    for (const tx of transactions) {
      const month = tx.tradeDate?.slice(0, 7) ?? "unknown";
      const symbol = tx.symbol ?? "\u2014";
      let symbolMap = map.get(month);
      if (!symbolMap) {
        symbolMap = new Map<string, BrokerageTransaction[]>();
        map.set(month, symbolMap);
      }
      const arr = symbolMap.get(symbol);
      if (arr) {
        arr.push(tx);
      } else {
        symbolMap.set(symbol, [tx]);
      }
    }
    return map;
  }, [transactions]);

  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    new Set(),
  );

  function toggleGroup(key: string) {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  // Convert selectedIds Set to TanStack Table's rowSelection format
  const rowSelection: Record<string, boolean> = {};
  transactions.forEach((tx, idx) => {
    if (selectedIds.has(tx.id)) {
      rowSelection[String(idx)] = true;
    }
  });

  const table = useReactTable({
    data: transactions,
    columns,
    state: { sorting, rowSelection },
    onSortingChange: setSorting,
    onRowSelectionChange: (updater) => {
      const next =
        typeof updater === "function" ? updater(rowSelection) : updater;
      // Start from existing selections so cross-page picks are preserved
      const newIds = new Set(selectedIds);
      // Reconcile only the rows visible on the current page
      transactions.forEach((tx, idx) => {
        if (next[String(idx)]) {
          newIds.add(tx.id);
        } else {
          newIds.delete(tx.id);
        }
      });
      onSelectedIdsChange(newIds);
    },
    enableRowSelection: (row) => !linkedTransactionIds.has(row.original.id),
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <div className="flex flex-1 flex-col gap-3">
      {/* Header row — always visible, even while loading */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {onSymbolSearchChange && (
            <Input
              placeholder="Search symbol..."
              value={symbolSearch}
              onChange={(e) => onSymbolSearchChange(e.target.value)}
              className="h-7 w-44"
            />
          )}
          <span className="text-xs text-muted-foreground">
            {isLoading ? "\u00A0" : `${total.toLocaleString()} transactions`}
          </span>
          {onDateRangeChange && (
            <div className="flex items-center rounded-md border bg-muted/30 p-0.5">
              {RANGE_PRESETS.map((preset) => (
                <button
                  key={preset.value}
                  onClick={() => onDateRangeChange(preset.value)}
                  className={`rounded px-2 py-0.5 text-[0.65rem] font-medium transition-colors ${
                    dateRange === preset.value
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          )}
        </div>
        <Select
          value={String(pageSize)}
          onValueChange={(v) => {
            onPageSizeChange(Number(v));
          }}
        >
          <SelectTrigger className="w-20 h-7">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="50">50</SelectItem>
            <SelectItem value="100">100</SelectItem>
            <SelectItem value="1000">1000</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {isLoading ? (
        <BrokerageTableSkeleton />
      ) : (
        <>
          {/* Table */}
          <div className="rounded-xl border">
            <table className="w-full text-xs">
              <thead>
                {table.getHeaderGroups().map((hg) => (
                  <tr key={hg.id} className="border-b bg-muted/50">
                    {hg.headers.map((h) => (
                      <th
                        key={h.id}
                        className="px-3 py-2 text-left font-medium text-muted-foreground"
                      >
                        {flexRender(h.column.columnDef.header, h.getContext())}
                      </th>
                    ))}
                  </tr>
                ))}
              </thead>
              <tbody>
                {transactions.length === 0 ? (
                  <tr>
                    <td
                      colSpan={columns.length}
                      className="px-3 py-12 text-center text-muted-foreground"
                    >
                      No transactions found.
                    </td>
                  </tr>
                ) : (
                  [...monthGroups.entries()].map(([month, symbolMap]) => {
                    const monthCollapsed = collapsedGroups.has(
                      `month:${month}`,
                    );
                    const allMonthTxs: BrokerageTransaction[] = [
                      ...symbolMap.values(),
                    ].flat();
                    const monthNet = groupNetAmount(allMonthTxs);
                    const monthIds = allMonthTxs
                      .filter((t) => !linkedTransactionIds.has(t.id))
                      .map((t) => t.id);
                    const monthAllSelected =
                      monthIds.length > 0 &&
                      monthIds.every((id) => selectedIds.has(id));
                    const monthSomeSelected = monthIds.some((id) =>
                      selectedIds.has(id),
                    );

                    return (
                      <MonthSection
                        key={month}
                        month={month}
                        tradeCount={allMonthTxs.length}
                        netAmount={monthNet}
                        currency={allMonthTxs[0]?.currency ?? "USD"}
                        isCollapsed={monthCollapsed}
                        onToggle={() => toggleGroup(`month:${month}`)}
                        allSelected={monthAllSelected}
                        someSelected={monthSomeSelected}
                        onSelectAll={(checked: boolean) => {
                          const newIds = new Set(selectedIds);
                          for (const id of monthIds) {
                            if (checked) newIds.add(id);
                            else newIds.delete(id);
                          }
                          onSelectedIdsChange(newIds);
                        }}
                        colSpan={columns.length}
                      >
                        {[...symbolMap.entries()].map(([symbol, txs]) => {
                          const symKey = `${month}:${symbol}`;
                          const symCollapsed = collapsedGroups.has(symKey);
                          const net = groupNetAmount(txs);
                          const desc = txs[0]?.symbolDescription;
                          const groupIds = txs
                            .filter((t) => !linkedTransactionIds.has(t.id))
                            .map((t) => t.id);
                          const allSelected =
                            groupIds.length > 0 &&
                            groupIds.every((id: string) => selectedIds.has(id));
                          const someSelected = groupIds.some((id: string) =>
                            selectedIds.has(id),
                          );

                          return (
                            <SymbolGroup
                              key={symKey}
                              symbol={symbol}
                              description={desc ?? undefined}
                              tradeCount={txs.length}
                              netAmount={net}
                              currency={txs[0]?.currency ?? "USD"}
                              isCollapsed={symCollapsed}
                              onToggle={() => toggleGroup(symKey)}
                              allSelected={allSelected}
                              someSelected={someSelected}
                              onSelectAll={(checked: boolean) => {
                                const newIds = new Set(selectedIds);
                                for (const id of groupIds) {
                                  if (checked) newIds.add(id);
                                  else newIds.delete(id);
                                }
                                onSelectedIdsChange(newIds);
                              }}
                              colSpan={columns.length}
                            >
                              {txs.map((tx) => {
                                const row = table
                                  .getRowModel()
                                  .rows.find((r) => r.original.id === tx.id);
                                if (!row) return null;
                                return (
                                  <tr
                                    key={row.id}
                                    className="border-b last:border-0"
                                  >
                                    {row.getVisibleCells().map((cell) => (
                                      <td key={cell.id} className="px-3 py-2">
                                        {flexRender(
                                          cell.column.columnDef.cell,
                                          cell.getContext(),
                                        )}
                                      </td>
                                    ))}
                                  </tr>
                                );
                              })}
                            </SymbolGroup>
                          );
                        })}
                      </MonthSection>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          {(hasPrevPage || hasNextPage) && (
            <div className="flex items-center justify-center gap-1.5 text-xs">
              <Button
                variant="outline"
                size="xs"
                disabled={!hasPrevPage}
                onClick={onPrevPage}
              >
                Previous
              </Button>
              <span className="px-2 text-muted-foreground">
                Page {page + 1}
              </span>
              <Button
                variant="outline"
                size="xs"
                disabled={!hasNextPage}
                onClick={onNextPage}
              >
                Next
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Loading skeleton
// ---------------------------------------------------------------------------

function BrokerageTableSkeleton() {
  return (
    <div className="rounded-xl border">
      <div className="border-b bg-muted/50 px-3 py-2">
        <Skeleton className="h-4 w-full" />
      </div>
      {Array.from({ length: 8 }).map((_, i) => (
        <div key={i} className="border-b px-3 py-3 last:border-0">
          <Skeleton className="h-4 w-full" />
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Month section header (collapsible)
// ---------------------------------------------------------------------------

function fmtMonth(key: string): string {
  if (key === "unknown") return "Unknown Date";
  const [year, month] = key.split("-");
  const date = new Date(Number(year), Number(month) - 1);
  return date.toLocaleDateString("en-US", { month: "long", year: "numeric" });
}

function MonthSection({
  month,
  tradeCount,
  netAmount,
  currency,
  isCollapsed,
  onToggle,
  allSelected,
  someSelected,
  onSelectAll,
  colSpan,
  children,
}: {
  month: string;
  tradeCount: number;
  netAmount: number;
  currency: string;
  isCollapsed: boolean;
  onToggle: () => void;
  allSelected: boolean;
  someSelected: boolean;
  onSelectAll: (checked: boolean) => void;
  colSpan: number;
  children: React.ReactNode;
}) {
  return (
    <>
      <tr
        className="border-b bg-muted/60 cursor-pointer hover:bg-muted/80 transition-colors"
        onClick={onToggle}
      >
        <td className="px-3 py-2" onClick={(e) => e.stopPropagation()}>
          <input
            type="checkbox"
            className="size-3.5 rounded border-muted-foreground/30"
            checked={allSelected}
            ref={(el) => {
              if (el) el.indeterminate = someSelected && !allSelected;
            }}
            onChange={(e) => onSelectAll(e.target.checked)}
          />
        </td>
        <td colSpan={colSpan - 1} className="px-3 py-2">
          <div className="flex items-center gap-3">
            <span className="text-[0.65rem] text-muted-foreground">
              {isCollapsed ? "\u25B6" : "\u25BC"}
            </span>
            <span className="font-semibold text-xs">{fmtMonth(month)}</span>
            <span className="text-[0.65rem] text-muted-foreground">
              {tradeCount} {tradeCount === 1 ? "trade" : "trades"}
            </span>
            <span className={`text-xs ${amountClasses(netAmount)}`}>
              {fmtCurrency(netAmount, currency)}
            </span>
          </div>
        </td>
      </tr>
      {!isCollapsed && children}
    </>
  );
}

// ---------------------------------------------------------------------------
// Symbol group within a month (collapsible)
// ---------------------------------------------------------------------------

function SymbolGroup({
  symbol,
  description,
  tradeCount,
  netAmount,
  currency,
  isCollapsed,
  onToggle,
  allSelected,
  someSelected,
  onSelectAll,
  colSpan,
  children,
}: {
  symbol: string;
  description?: string;
  tradeCount: number;
  netAmount: number;
  currency: string;
  isCollapsed: boolean;
  onToggle: () => void;
  allSelected: boolean;
  someSelected: boolean;
  onSelectAll: (checked: boolean) => void;
  colSpan: number;
  children: React.ReactNode;
}) {
  return (
    <>
      <tr
        className="border-b bg-muted/20 cursor-pointer hover:bg-muted/40 transition-colors"
        onClick={onToggle}
      >
        <td className="px-3 py-1.5" onClick={(e) => e.stopPropagation()}>
          <input
            type="checkbox"
            className="size-3.5 rounded border-muted-foreground/30"
            checked={allSelected}
            ref={(el) => {
              if (el) el.indeterminate = someSelected && !allSelected;
            }}
            onChange={(e) => onSelectAll(e.target.checked)}
          />
        </td>
        <td colSpan={colSpan - 1} className="px-3 py-1.5">
          <div className="flex items-center gap-3 pl-4">
            <span className="text-[0.6rem] text-muted-foreground">
              {isCollapsed ? "\u25B6" : "\u25BC"}
            </span>
            <span className="font-medium text-xs">{symbol}</span>
            {description && (
              <span className="text-[0.65rem] text-muted-foreground truncate max-w-[12rem]">
                {description}
              </span>
            )}
            <span className="text-[0.65rem] text-muted-foreground">
              {tradeCount} {tradeCount === 1 ? "trade" : "trades"}
            </span>
            <span className={`text-xs ${amountClasses(netAmount)}`}>
              {fmtCurrency(netAmount, currency)}
            </span>
          </div>
        </td>
      </tr>
      {!isCollapsed && children}
    </>
  );
}
