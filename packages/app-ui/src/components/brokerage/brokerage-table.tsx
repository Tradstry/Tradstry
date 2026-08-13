"use client";

import {
  ArrowDown01Icon,
  ArrowLeft01Icon,
  ArrowRight01Icon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { Checkbox } from "@tradstry/app-ui/components/ui/checkbox";
import { Input } from "@tradstry/app-ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { Skeleton } from "@tradstry/app-ui/components/ui/skeleton";
import { RANGE_PRESETS } from "@tradstry/app-ui/lib/range-presets";
import type { AnalyticsRange } from "@tradstry/app-ui/lib/types/analytics";
import type { BrokerageTransaction } from "@tradstry/app-ui/lib/types/brokerage";
import { cn } from "@tradstry/app-ui/lib/utils";
import { useId, useMemo, useState } from "react";

const TYPE_COLORS: Record<string, string> = {
  BUY: "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/50 dark:text-emerald-300",
  SELL: "border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900 dark:bg-rose-950/50 dark:text-rose-300",
  DIVIDEND:
    "border-indigo-200 bg-indigo-50 text-indigo-700 dark:border-indigo-900 dark:bg-indigo-950/50 dark:text-indigo-300",
  STOCK_DIVIDEND:
    "border-indigo-200 bg-indigo-50 text-indigo-700 dark:border-indigo-900 dark:bg-indigo-950/50 dark:text-indigo-300",
  INTEREST:
    "border-indigo-200 bg-indigo-50 text-indigo-700 dark:border-indigo-900 dark:bg-indigo-950/50 dark:text-indigo-300",
  REI: "border-indigo-200 bg-indigo-50 text-indigo-700 dark:border-indigo-900 dark:bg-indigo-950/50 dark:text-indigo-300",
  OPTIONEXPIRATION:
    "border-violet-200 bg-violet-50 text-violet-700 dark:border-violet-900 dark:bg-violet-950/50 dark:text-violet-300",
  OPTIONASSIGNMENT:
    "border-violet-200 bg-violet-50 text-violet-700 dark:border-violet-900 dark:bg-violet-950/50 dark:text-violet-300",
  OPTIONEXERCISE:
    "border-violet-200 bg-violet-50 text-violet-700 dark:border-violet-900 dark:bg-violet-950/50 dark:text-violet-300",
  TRANSFER:
    "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/50 dark:text-amber-300",
  CONTRIBUTION:
    "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/50 dark:text-amber-300",
  WITHDRAWAL:
    "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/50 dark:text-amber-300",
  EXTERNAL_ASSET_TRANSFER_IN:
    "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/50 dark:text-amber-300",
  EXTERNAL_ASSET_TRANSFER_OUT:
    "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/50 dark:text-amber-300",
};

const DEFAULT_TYPE_COLOR = "border-border bg-muted text-muted-foreground";
const TABLE_SKELETON_ROWS = [
  "ledger-skeleton-1",
  "ledger-skeleton-2",
  "ledger-skeleton-3",
  "ledger-skeleton-4",
  "ledger-skeleton-5",
  "ledger-skeleton-6",
  "ledger-skeleton-7",
  "ledger-skeleton-8",
  "ledger-skeleton-9",
  "ledger-skeleton-10",
];

function TypeBadge({ type }: { type: string }) {
  return (
    <span
      className={cn(
        "inline-flex max-w-40 truncate rounded-full border px-2 py-0.5 text-[0.625rem] font-semibold tracking-[0.04em]",
        TYPE_COLORS[type] ?? DEFAULT_TYPE_COLOR,
      )}
      title={type.replaceAll("_", " ")}
    >
      {type.replaceAll("_", " ")}
    </span>
  );
}

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
    ? "font-semibold text-rose-600 dark:text-rose-400"
    : "font-semibold text-emerald-600 dark:text-emerald-400";
}

function groupNetAmount(txs: BrokerageTransaction[]): number {
  return txs.reduce((sum, tx) => sum + (tx.amount ?? 0), 0);
}

function buildColumns(
  linkedIds: Set<string>,
): ColumnDef<BrokerageTransaction>[] {
  return [
    {
      id: "select",
      header: ({ table }) => (
        <Checkbox
          aria-label="Select all visible transactions"
          checked={
            table.getIsAllPageRowsSelected()
              ? true
              : table.getIsSomePageRowsSelected()
                ? "indeterminate"
                : false
          }
          onCheckedChange={(checked) =>
            table.toggleAllPageRowsSelected(checked === true)
          }
        />
      ),
      cell: ({ row }) => {
        const isLinked = linkedIds.has(row.original.id);
        return (
          <div className="flex items-center gap-2">
            <Checkbox
              aria-label={`Select ${row.original.symbol ?? "transaction"}`}
              checked={row.getIsSelected()}
              disabled={isLinked}
              onCheckedChange={(checked) =>
                row.toggleSelected(checked === true)
              }
            />
            {isLinked && (
              <span className="hidden rounded-full border border-indigo-200 bg-indigo-50 px-1.5 py-0.5 text-[0.5625rem] font-semibold text-indigo-600 dark:border-indigo-900 dark:bg-indigo-950/50 dark:text-indigo-300 xl:inline-flex">
                In journal
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
        <span className="whitespace-nowrap text-muted-foreground">
          {fmtDate(row.original.tradeDate)}
        </span>
      ),
    },
    {
      accessorKey: "symbol",
      header: "Security",
      cell: ({ row }) => {
        const { symbol, symbolDescription } = row.original;
        if (!symbol) {
          return <span className="text-muted-foreground">{"\u2014"}</span>;
        }
        return (
          <div className="flex min-w-0 flex-col">
            <span className="font-mono text-xs font-semibold tracking-wide">
              {symbol}
            </span>
            {symbolDescription && (
              <span className="max-w-56 truncate text-[0.6875rem] text-muted-foreground">
                {symbolDescription}
              </span>
            )}
          </div>
        );
      },
    },
    {
      accessorKey: "transactionType",
      header: "Side",
      cell: ({ row }) => <TypeBadge type={row.original.transactionType} />,
    },
    {
      accessorKey: "units",
      header: "Quantity",
      cell: ({ row }) => {
        const value = row.original.units;
        return value !== 0 ? Math.abs(value).toLocaleString() : "\u2014";
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

interface BrokerageTableProps {
  transactions: BrokerageTransaction[];
  total: number;
  offset: number;
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
  onSymbolSearchChange?: (value: string) => void;
  scopeControl?: React.ReactNode;
}

const NUMERIC_COLUMNS = new Set(["units", "price", "amount", "fee"]);

const LEDGER_COLUMN_WIDTHS = [
  { id: "select", width: "7%" },
  { id: "tradeDate", width: "9%" },
  { id: "symbol", width: "22%" },
  { id: "transactionType", width: "10%" },
  { id: "units", width: "13%" },
  { id: "price", width: "13%" },
  { id: "amount", width: "15%" },
  { id: "fee", width: "11%" },
] as const;

function columnClass(columnId: string, header = false): string {
  return cn(
    "overflow-hidden px-3",
    header ? "h-10 py-2" : "py-2.5",
    NUMERIC_COLUMNS.has(columnId) &&
      "whitespace-nowrap text-right font-mono tabular-nums",
  );
}

export function BrokerageTable({
  transactions,
  total,
  offset,
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
  scopeControl,
}: BrokerageTableProps) {
  const symbolSearchId = useId();
  const columns = useMemo(
    () => buildColumns(linkedTransactionIds),
    [linkedTransactionIds],
  );

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
      const group = symbolMap.get(symbol);
      if (group) group.push(tx);
      else symbolMap.set(symbol, [tx]);
    }
    return map;
  }, [transactions]);

  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    new Set(),
  );

  function toggleGroup(key: string) {
    setCollapsedGroups((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  const rowSelection: Record<string, boolean> = {};
  transactions.forEach((tx, index) => {
    if (selectedIds.has(tx.id)) rowSelection[String(index)] = true;
  });

  const table = useReactTable({
    data: transactions,
    columns,
    state: { rowSelection },
    onRowSelectionChange: (updater) => {
      const next =
        typeof updater === "function" ? updater(rowSelection) : updater;
      const newIds = new Set(selectedIds);
      transactions.forEach((tx, index) => {
        if (next[String(index)]) newIds.add(tx.id);
        else newIds.delete(tx.id);
      });
      onSelectedIdsChange(newIds);
    },
    enableRowSelection: (row) => !linkedTransactionIds.has(row.original.id),
    getCoreRowModel: getCoreRowModel(),
  });

  const visibleStart = total === 0 ? 0 : offset + 1;
  const visibleEnd = Math.min(offset + transactions.length, total);

  return (
    <section
      aria-label="Brokerage execution ledger"
      className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border/80 bg-background shadow-[0_1px_2px_rgb(0_0_0/0.04),0_10px_30px_rgb(0_0_0/0.025)]"
    >
      <div className="shrink-0 border-b bg-muted/15 px-3 py-3 md:px-4">
        <div className="flex flex-wrap items-center gap-2.5">
          <div className="mr-1 min-w-36">
            <h2 className="text-sm font-semibold tracking-tight">
              Execution ledger
            </h2>
            <p className="text-[0.6875rem] text-muted-foreground">
              {isLoading
                ? "Loading fills\u2026"
                : `${total.toLocaleString()} fills`}
            </p>
          </div>

          {scopeControl}

          {onSymbolSearchChange && (
            <label
              htmlFor={symbolSearchId}
              className="relative min-w-48 flex-1 sm:max-w-64"
            >
              <span className="sr-only">Search by symbol</span>
              <HugeiconsIcon
                icon={Search01Icon}
                strokeWidth={1.8}
                className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
              />
              <Input
                id={symbolSearchId}
                placeholder="Search ticker"
                value={symbolSearch}
                onChange={(event) => onSymbolSearchChange(event.target.value)}
                className="h-8 rounded-lg bg-background pl-8 shadow-none"
              />
            </label>
          )}

          {onDateRangeChange && (
            <fieldset className="ml-auto flex max-w-full items-center gap-0.5 overflow-x-auto rounded-lg border bg-background p-0.5">
              <legend className="sr-only">Date range</legend>
              {RANGE_PRESETS.map((preset) => (
                <button
                  key={preset.value}
                  type="button"
                  aria-pressed={dateRange === preset.value}
                  onClick={() => onDateRangeChange(preset.value)}
                  className={cn(
                    "h-6 shrink-0 rounded-md px-2 text-[0.625rem] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/30",
                    dateRange === preset.value
                      ? "bg-foreground text-background shadow-sm"
                      : "text-muted-foreground hover:bg-muted hover:text-foreground",
                  )}
                >
                  {preset.label}
                </button>
              ))}
            </fieldset>
          )}
        </div>
      </div>

      {isLoading ? (
        <BrokerageTableSkeleton />
      ) : (
        <>
          <div className="min-h-0 flex-1 overflow-auto overscroll-contain">
            <table className="w-full min-w-[58rem] table-fixed border-separate border-spacing-0 text-xs">
              <colgroup>
                {LEDGER_COLUMN_WIDTHS.map((column) => (
                  <col key={column.id} style={{ width: column.width }} />
                ))}
              </colgroup>
              <thead className="sticky top-0 z-20 bg-background/95 shadow-[0_1px_0_var(--border)] backdrop-blur-sm">
                {table.getHeaderGroups().map((headerGroup) => (
                  <tr key={headerGroup.id}>
                    {headerGroup.headers.map((header) => (
                      <th
                        key={header.id}
                        scope="col"
                        className={cn(
                          columnClass(header.column.id, true),
                          "text-left text-[0.625rem] font-semibold uppercase tracking-[0.08em] text-muted-foreground",
                        )}
                      >
                        {flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )}
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
                      className="px-6 py-20 text-center"
                    >
                      <p className="text-sm font-medium">No matching fills</p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        Try another ticker, date range, or journal status.
                      </p>
                    </td>
                  </tr>
                ) : (
                  [...monthGroups.entries()].map(([month, symbolMap]) => {
                    const monthCollapsed = collapsedGroups.has(
                      `month:${month}`,
                    );
                    const allMonthTxs = [...symbolMap.values()].flat();
                    const monthIds = allMonthTxs
                      .filter((tx) => !linkedTransactionIds.has(tx.id))
                      .map((tx) => tx.id);
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
                        netAmount={groupNetAmount(allMonthTxs)}
                        currency={allMonthTxs[0]?.currency ?? "USD"}
                        isCollapsed={monthCollapsed}
                        onToggle={() => toggleGroup(`month:${month}`)}
                        allSelected={monthAllSelected}
                        someSelected={monthSomeSelected}
                        onSelectAll={(checked) => {
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
                          const symbolKey = `${month}:${symbol}`;
                          const symbolCollapsed =
                            collapsedGroups.has(symbolKey);
                          const groupIds = txs
                            .filter((tx) => !linkedTransactionIds.has(tx.id))
                            .map((tx) => tx.id);
                          const allSelected =
                            groupIds.length > 0 &&
                            groupIds.every((id) => selectedIds.has(id));
                          const someSelected = groupIds.some((id) =>
                            selectedIds.has(id),
                          );

                          return (
                            <SymbolGroup
                              key={symbolKey}
                              symbol={symbol}
                              description={
                                txs[0]?.symbolDescription ?? undefined
                              }
                              tradeCount={txs.length}
                              netAmount={groupNetAmount(txs)}
                              currency={txs[0]?.currency ?? "USD"}
                              isCollapsed={symbolCollapsed}
                              onToggle={() => toggleGroup(symbolKey)}
                              allSelected={allSelected}
                              someSelected={someSelected}
                              onSelectAll={(checked) => {
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
                                  .rows.find(
                                    (candidate) =>
                                      candidate.original.id === tx.id,
                                  );
                                if (!row) return null;
                                return (
                                  <tr
                                    key={row.id}
                                    className={cn(
                                      "group border-b transition-colors hover:bg-muted/35",
                                      row.getIsSelected() &&
                                        "bg-sky-50/70 dark:bg-sky-950/20",
                                    )}
                                  >
                                    {row.getVisibleCells().map((cell) => (
                                      <td
                                        key={cell.id}
                                        className={cn(
                                          columnClass(cell.column.id),
                                          "border-b border-border/60 group-last:border-b-0",
                                        )}
                                      >
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

          <footer className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-t bg-muted/10 px-3 py-2 md:px-4">
            <p className="text-[0.6875rem] text-muted-foreground tabular-nums">
              Showing{" "}
              <span className="font-medium text-foreground">
                {visibleStart}
                {"–"}
                {visibleEnd}
              </span>{" "}
              of{" "}
              <span className="font-medium text-foreground">
                {total.toLocaleString()}
              </span>
            </p>

            <div className="flex items-center gap-3">
              <div className="flex items-center gap-2 text-[0.6875rem] text-muted-foreground">
                <span>Rows</span>
                <Select
                  value={String(pageSize)}
                  onValueChange={(value) => onPageSizeChange(Number(value))}
                >
                  <SelectTrigger className="h-7 w-20 bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="50">50</SelectItem>
                    <SelectItem value="100">100</SelectItem>
                    <SelectItem value="1000">1000</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="flex items-center gap-1">
                <Button
                  variant="outline"
                  size="icon-sm"
                  aria-label="Previous page"
                  disabled={!hasPrevPage}
                  onClick={onPrevPage}
                >
                  <HugeiconsIcon icon={ArrowLeft01Icon} strokeWidth={1.8} />
                </Button>
                <span className="min-w-16 text-center text-[0.6875rem] text-muted-foreground tabular-nums">
                  Page {page + 1}
                </span>
                <Button
                  variant="outline"
                  size="icon-sm"
                  aria-label="Next page"
                  disabled={!hasNextPage}
                  onClick={onNextPage}
                >
                  <HugeiconsIcon icon={ArrowRight01Icon} strokeWidth={1.8} />
                </Button>
              </div>
            </div>
          </footer>
        </>
      )}
    </section>
  );
}

function BrokerageTableSkeleton() {
  return (
    <div className="min-h-0 flex-1 overflow-hidden" aria-live="polite">
      <span className="sr-only">Loading transactions</span>
      <div className="border-b px-3 py-3">
        <Skeleton className="h-4 w-full" />
      </div>
      {TABLE_SKELETON_ROWS.map((row, index) => (
        <div
          key={row}
          className="border-b border-border/60 px-3 py-3 last:border-0"
        >
          <Skeleton
            className={cn("h-4", index % 3 === 0 ? "w-4/5" : "w-full")}
          />
        </div>
      ))}
    </div>
  );
}

function fmtMonth(key: string): string {
  if (key === "unknown") return "Unknown date";
  const [year, month] = key.split("-");
  const date = new Date(Number(year), Number(month) - 1);
  return date.toLocaleDateString("en-US", { month: "long", year: "numeric" });
}

function GroupCheckbox({
  label,
  allSelected,
  someSelected,
  onSelectAll,
}: {
  label: string;
  allSelected: boolean;
  someSelected: boolean;
  onSelectAll: (checked: boolean) => void;
}) {
  return (
    <Checkbox
      aria-label={label}
      checked={allSelected ? true : someSelected ? "indeterminate" : false}
      onCheckedChange={(checked) => onSelectAll(checked === true)}
    />
  );
}

function DisclosureIcon({ isCollapsed }: { isCollapsed: boolean }) {
  return (
    <HugeiconsIcon
      icon={ArrowDown01Icon}
      strokeWidth={2}
      className={cn(
        "size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 motion-reduce:transition-none",
        isCollapsed && "-rotate-90",
      )}
    />
  );
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
  const label = fmtMonth(month);
  return (
    <>
      <tr className="bg-muted/65">
        <td className="border-b border-l-2 border-l-sky-500 px-3 py-2.5">
          <GroupCheckbox
            label={`Select all fills in ${label}`}
            allSelected={allSelected}
            someSelected={someSelected}
            onSelectAll={onSelectAll}
          />
        </td>
        <td colSpan={colSpan - 1} className="border-b py-0 pr-3">
          <button
            type="button"
            aria-expanded={!isCollapsed}
            onClick={onToggle}
            className="flex w-full items-center gap-2.5 py-2.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/30"
          >
            <DisclosureIcon isCollapsed={isCollapsed} />
            <span className="text-xs font-semibold tracking-tight">
              {label}
            </span>
            <span className="rounded-full border border-border/70 bg-background/70 px-1.5 py-0.5 text-[0.625rem] text-muted-foreground">
              {tradeCount} {tradeCount === 1 ? "fill" : "fills"}
            </span>
            <span
              className={cn(
                "ml-auto font-mono text-xs tabular-nums",
                amountClasses(netAmount),
              )}
            >
              {fmtCurrency(netAmount, currency)}
            </span>
          </button>
        </td>
      </tr>
      {!isCollapsed && children}
    </>
  );
}

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
      <tr className="bg-muted/20">
        <td className="border-b border-border/60 px-3 py-2">
          <GroupCheckbox
            label={`Select all ${symbol} fills`}
            allSelected={allSelected}
            someSelected={someSelected}
            onSelectAll={onSelectAll}
          />
        </td>
        <td
          colSpan={colSpan - 1}
          className="border-b border-border/60 py-0 pr-3"
        >
          <button
            type="button"
            aria-expanded={!isCollapsed}
            onClick={onToggle}
            className="flex w-full items-center gap-2.5 py-2 pl-5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/30"
          >
            <DisclosureIcon isCollapsed={isCollapsed} />
            <span className="inline-flex min-w-14 justify-center rounded-md bg-foreground px-2 py-1 font-mono text-[0.6875rem] font-semibold tracking-wide text-background">
              {symbol}
            </span>
            {description && (
              <span className="max-w-64 truncate text-[0.6875rem] text-muted-foreground">
                {description}
              </span>
            )}
            <span className="text-[0.625rem] text-muted-foreground">
              {tradeCount} {tradeCount === 1 ? "fill" : "fills"}
            </span>
            <span
              className={cn(
                "ml-auto font-mono text-xs tabular-nums",
                amountClasses(netAmount),
              )}
            >
              {fmtCurrency(netAmount, currency)}
            </span>
          </button>
        </td>
      </tr>
      {!isCollapsed && children}
    </>
  );
}
