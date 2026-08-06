import { type ComponentProps, useCallback, useEffect, useMemo, useState } from "react";
import {
  type ColumnDef,
  type ColumnFiltersState,
  type FilterFn,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { PencilSimpleIcon, TrashIcon } from "@phosphor-icons/react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  accounts,
  deleteJournalEntry,
  journalEntries,
  tagCategories as fetchTagCategories,
  type JournalEntry,
  type TagCategory,
} from "../../backend";
import { TradeFormDialog } from "./trade-form";
import { TagManager } from "./tag-manager";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

const percentFormatter = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});
const dateFormatter = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "2-digit",
  year: "2-digit",
});
const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});

function formatDate(value: string) {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : dateFormatter.format(parsed);
}
function formatPercent(value: number) {
  return `${value > 0 ? "+" : ""}${percentFormatter.format(value)}%`;
}
function formatCurrency(value: number) {
  return `${value >= 0 ? "+" : ""}${usd.format(value)}`;
}
function formatDuration(seconds: number) {
  if (seconds <= 0) return "0m";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

function statusClasses(status: JournalEntry["status"]) {
  return status === "profit"
    ? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/50 dark:text-emerald-300"
    : "border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900 dark:bg-rose-950/50 dark:text-rose-300";
}
function tradeTypeClasses(type: JournalEntry["tradeType"]) {
  return type === "long"
    ? "border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900 dark:bg-sky-950/50 dark:text-sky-300"
    : "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/50 dark:text-amber-300";
}
function valueClasses(value: number) {
  return value >= 0
    ? "text-emerald-600 dark:text-emerald-400"
    : "text-rose-600 dark:text-rose-400";
}

const rowSearchFilter: FilterFn<JournalEntry> = (row, _columnId, filterValue) => {
  const search = String(filterValue ?? "")
    .trim()
    .toLowerCase();
  if (!search) return true;
  const haystack = [
    row.original.symbol,
    row.original.symbolName,
    row.original.tradeType,
    row.original.status,
    row.original.notes ?? "",
    ...row.original.tags.map((t) => t.name),
  ]
    .join(" ")
    .toLowerCase();
  return haystack.includes(search);
};

function SortableHeader({
  label,
  canSort,
  sortDirection,
  onClick,
}: {
  label: string;
  canSort: boolean;
  sortDirection: false | "asc" | "desc";
  onClick?: ComponentProps<"button">["onClick"];
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1 text-left font-medium",
        canSort
          ? "cursor-pointer text-foreground"
          : "cursor-default text-muted-foreground",
      )}
    >
      <span>{label}</span>
      {sortDirection === "asc" ? <span>↑</span> : null}
      {sortDirection === "desc" ? <span>↓</span> : null}
    </button>
  );
}

function MetricCard({
  label,
  value,
  sublabel,
}: {
  label: string;
  value: string;
  sublabel: string;
}) {
  return (
    <div className="rounded-xl border bg-background p-4">
      <p className="text-[0.68rem] font-semibold uppercase tracking-[0.2em] text-muted-foreground">
        {label}
      </p>
      <p className="mt-2 text-2xl font-semibold text-foreground">{value}</p>
      <p className="mt-1 text-xs text-muted-foreground">{sublabel}</p>
    </div>
  );
}

function DeleteTradeButton({
  symbol,
  onConfirm,
}: {
  symbol: string;
  onConfirm: () => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Delete trade"
          className="text-muted-foreground hover:text-destructive"
        >
          <TrashIcon size={15} />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-56 gap-3">
        <p className="text-sm text-foreground">Delete this {symbol} trade?</p>
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button
            size="sm"
            className="border-transparent bg-destructive text-white hover:bg-destructive/90"
            onClick={() => {
              onConfirm();
              setOpen(false);
            }}
          >
            Delete
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function JournalTableLoading() {
  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-3">
        {["a", "b", "c"].map((key) => (
          <Skeleton key={key} className="h-24 rounded-xl" />
        ))}
      </div>
      <Skeleton className="h-14 rounded-xl" />
      <Skeleton className="h-[28rem] rounded-xl" />
    </div>
  );
}

export default function JournalTrades() {
  const reduce = useReducedMotion();
  const [rows, setRows] = useState<JournalEntry[]>([]);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [categories, setCategories] = useState<TagCategory[]>([]);

  const [sorting, setSorting] = useState<SortingState>([
    { id: "openDate", desc: true },
  ]);
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [search, setSearch] = useState("");

  const reload = useCallback(() => {
    if (!accountId) return;
    journalEntries(accountId)
      .then((r) => {
        setRows(r);
        setState("ready");
      })
      .catch((e) => {
        setError(String(e));
        setState("error");
      });
  }, [accountId]);

  useEffect(() => {
    accounts()
      .then((a) => setAccountId(a[0]?.id ?? null))
      .catch(() => setState("error"));
    fetchTagCategories().then(setCategories).catch(() => {});
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const handleDelete = useCallback(
    (id: string) => {
      deleteJournalEntry(id)
        .then(reload)
        .catch((e) => setError(String(e)));
    },
    [reload],
  );

  const columns = useMemo<ColumnDef<JournalEntry>[]>(
    () => [
      {
        accessorKey: "openDate",
        header: ({ column }) => (
          <SortableHeader
            label="Open Date"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <div className="space-y-1 whitespace-nowrap">
            <p className="font-medium text-foreground">
              {formatDate(row.original.openDate)}
            </p>
            <p className="text-xs text-muted-foreground">
              {formatDate(row.original.closeDate)}
            </p>
          </div>
        ),
      },
      {
        accessorKey: "symbol",
        header: ({ column }) => (
          <SortableHeader
            label="Symbol"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <div className="space-y-1">
            <p className="font-semibold uppercase tracking-[0.12em] text-foreground">
              {row.original.symbol}
            </p>
            {row.original.symbolName ? (
              <p className="max-w-[14rem] truncate text-xs text-muted-foreground">
                {row.original.symbolName}
              </p>
            ) : null}
          </div>
        ),
      },
      {
        accessorKey: "status",
        filterFn: "equalsString",
        header: ({ column }) => (
          <SortableHeader
            label="Status"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <span
            className={cn(
              "inline-flex rounded-full border px-2.5 py-1 text-[0.65rem] font-semibold uppercase tracking-[0.18em]",
              statusClasses(row.original.status),
            )}
          >
            {row.original.status}
          </span>
        ),
      },
      {
        accessorKey: "netRoi",
        header: ({ column }) => (
          <SortableHeader
            label="Net ROI"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <span className={cn("font-semibold", valueClasses(row.original.netRoi))}>
            {formatPercent(row.original.netRoi)}
          </span>
        ),
      },
      {
        accessorKey: "totalPl",
        header: ({ column }) => (
          <SortableHeader
            label="Net P/L"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <span
            className={cn("font-semibold", valueClasses(row.original.totalPl))}
          >
            {formatPercent(row.original.totalPl)}
          </span>
        ),
      },
      {
        accessorKey: "duration",
        header: ({ column }) => (
          <SortableHeader
            label="Duration"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">
            {formatDuration(row.original.duration)}
          </span>
        ),
      },
      {
        accessorKey: "riskReward",
        header: ({ column }) => (
          <SortableHeader
            label="R:R"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <span
            className={cn("font-medium", valueClasses(row.original.riskReward))}
          >
            {row.original.riskReward.toFixed(2)}R
          </span>
        ),
      },
      {
        accessorKey: "tradeType",
        filterFn: "equalsString",
        header: ({ column }) => (
          <SortableHeader
            label="Type"
            canSort={column.getCanSort()}
            sortDirection={column.getIsSorted()}
            onClick={column.getToggleSortingHandler()}
          />
        ),
        cell: ({ row }) => (
          <span
            className={cn(
              "inline-flex rounded-full border px-2.5 py-1 text-[0.65rem] font-semibold uppercase tracking-[0.18em]",
              tradeTypeClasses(row.original.tradeType),
            )}
          >
            {row.original.tradeType}
          </span>
        ),
      },
      {
        id: "tags",
        header: "Tags",
        cell: ({ row }) => {
          const t = row.original;
          const groups = categories
            .map((cat) => ({
              cat,
              catTags: t.tags.filter((tag) => tag.categoryId === cat.id),
            }))
            .filter((g) => g.catTags.length > 0);
          const ungrouped = t.tags.filter(
            (tag) => !categories.some((c) => c.id === tag.categoryId),
          );
          const hasAny = groups.length > 0 || ungrouped.length > 0;
          return (
            <div className="flex items-start justify-between gap-3">
              <div className="space-y-1">
                {hasAny ? (
                  <>
                    {groups.map(({ cat, catTags }) => (
                      <div
                        key={cat.id}
                        className="flex flex-wrap items-center gap-1"
                      >
                        <span className="text-[11px] font-medium text-muted-foreground">
                          {cat.name}
                        </span>
                        {catTags.map((tag) => (
                          <Badge
                            key={tag.id}
                            variant="outline"
                            className="border-transparent text-white"
                            style={{
                              backgroundColor:
                                tag.color ?? cat.color ?? "#94a3b8",
                            }}
                          >
                            {tag.name}
                          </Badge>
                        ))}
                      </div>
                    ))}
                    {ungrouped.length > 0 && (
                      <div className="flex flex-wrap gap-1">
                        {ungrouped.map((tag) => (
                          <Badge
                            key={tag.id}
                            variant="outline"
                            className="border-transparent text-white"
                            style={{ backgroundColor: tag.color ?? "#94a3b8" }}
                          >
                            {tag.name}
                          </Badge>
                        ))}
                      </div>
                    )}
                  </>
                ) : (
                  <span className="text-xs text-muted-foreground">—</span>
                )}
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <TradeFormDialog
                  entry={t}
                  accountId={accountId}
                  onSaved={reload}
                  trigger={
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label="Edit trade"
                    >
                      <PencilSimpleIcon size={15} />
                    </Button>
                  }
                />
                <DeleteTradeButton
                  symbol={t.symbol}
                  onConfirm={() => handleDelete(t.id)}
                />
              </div>
            </div>
          );
        },
      },
    ],
    [accountId, reload, handleDelete, categories],
  );

  const table = useReactTable({
    data: rows,
    columns,
    state: { sorting, columnFilters, globalFilter: search },
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    globalFilterFn: rowSearchFilter,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: { pagination: { pageIndex: 0, pageSize: 8 } },
  });

  const statusFilter =
    (table.getColumn("status")?.getFilterValue() as string | undefined) ?? "all";
  const tradeTypeFilter =
    (table.getColumn("tradeType")?.getFilterValue() as string | undefined) ??
    "all";
  const filteredRows = table.getFilteredRowModel().rows.map((r) => r.original);
  const totalTrades = filteredRows.length;
  const profitTrades = filteredRows.filter((e) => e.totalPl > 0).length;
  const lossTrades = filteredRows.filter((e) => e.totalPl < 0).length;
  const decisiveTrades = profitTrades + lossTrades;
  const cumulativeProfit = filteredRows.reduce(
    (sum, e) => sum + (e.positionSize * e.entryPrice * e.totalPl) / 100,
    0,
  );
  const averageRiskReward =
    totalTrades === 0
      ? 0
      : filteredRows.reduce((sum, e) => sum + e.riskReward, 0) / totalTrades;

  if (state === "loading") {
    return <JournalTableLoading />;
  }

  if (state === "error") {
    return (
      <section className="rounded-xl border border-rose-200 bg-rose-50 p-6 text-rose-700 dark:border-rose-900 dark:bg-rose-950/40 dark:text-rose-300">
        <p className="text-sm font-semibold uppercase tracking-[0.22em]">
          Journal Error
        </p>
        <p className="mt-2 text-sm">{error}</p>
      </section>
    );
  }

  return (
    <div className="space-y-4 pt-1">
      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        <MetricCard
          label="Cumulative Profit"
          value={formatCurrency(cumulativeProfit)}
          sublabel="Combined dollar P/L across trades"
        />
        <MetricCard
          label="Win Rate"
          value={
            decisiveTrades === 0
              ? "0.00%"
              : formatPercent((profitTrades / decisiveTrades) * 100)
          }
          sublabel={`${profitTrades} winning trades out of ${decisiveTrades}`}
        />
        <MetricCard
          label="Average R:R"
          value={`${averageRiskReward.toFixed(2)}R`}
          sublabel="Realized reward-to-risk across trades"
        />
      </div>

      <div className="rounded-xl border bg-background">
        <div className="flex flex-col gap-3 border-b border-border px-4 py-4 md:flex-row md:items-center md:justify-between">
          <div className="flex flex-1 flex-col gap-3 md:flex-row md:items-center">
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search symbol, notes, or tags"
              className="h-10 rounded-xl border-border bg-muted/50 text-sm md:max-w-xs"
            />
            <Select
              value={statusFilter}
              onValueChange={(value) =>
                table
                  .getColumn("status")
                  ?.setFilterValue(value === "all" ? undefined : value)
              }
            >
              <SelectTrigger className="h-10 w-full rounded-xl border-border bg-muted/50 md:w-[9rem]">
                <SelectValue placeholder="Status" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All statuses</SelectItem>
                <SelectItem value="profit">Profit</SelectItem>
                <SelectItem value="loss">Loss</SelectItem>
              </SelectContent>
            </Select>
            <Select
              value={tradeTypeFilter}
              onValueChange={(value) =>
                table
                  .getColumn("tradeType")
                  ?.setFilterValue(value === "all" ? undefined : value)
              }
            >
              <SelectTrigger className="h-10 w-full rounded-xl border-border bg-muted/50 md:w-[9rem]">
                <SelectValue placeholder="Trade type" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All types</SelectItem>
                <SelectItem value="long">Long</SelectItem>
                <SelectItem value="short">Short</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="flex items-center gap-2">
            <TradeFormDialog
              accountId={accountId}
              onSaved={reload}
              trigger={<Button size="sm">New trade</Button>}
            />
            <TagManager
              onChanged={() => {
                reload();
                fetchTagCategories().then(setCategories).catch(() => {});
              }}
            />
            <Button variant="outline" size="sm" onClick={() => reload()}>
              Refresh
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setSearch("");
                table.resetColumnFilters();
                table.resetSorting();
              }}
            >
              Reset
            </Button>
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="min-w-full text-sm">
            <thead>
              {table.getHeaderGroups().map((headerGroup) => (
                <tr key={headerGroup.id} className="border-b">
                  {headerGroup.headers.map((header) => (
                    <th
                      key={header.id}
                      className="px-4 py-3 text-left text-[0.68rem] font-semibold uppercase tracking-[0.2em] text-muted-foreground"
                    >
                      {header.isPlaceholder
                        ? null
                        : flexRender(
                            header.column.columnDef.header,
                            header.getContext(),
                          )}
                    </th>
                  ))}
                </tr>
              ))}
            </thead>
            <tbody>
              {table.getRowModel().rows.length === 0 ? (
                <tr>
                  <td colSpan={columns.length} className="px-4 py-14 text-center">
                    <p className="text-sm font-medium text-foreground">
                      No trades found
                    </p>
                    <p className="mt-2 text-sm text-muted-foreground">
                      Log a trade or connect a brokerage to populate your
                      journal.
                    </p>
                  </td>
                </tr>
              ) : (
                <AnimatePresence>
                  {table.getRowModel().rows.map((row, index) => (
                    <motion.tr
                      key={row.id}
                      initial={reduce ? { opacity: 0 } : { opacity: 0, y: 6 }}
                      animate={{
                        opacity: 1,
                        y: 0,
                        transition: {
                          duration: 0.2,
                          ease: "easeOut",
                          delay: Math.min(index, 8) * 0.025,
                        },
                      }}
                      exit={{ opacity: 0, transition: { duration: 0.15 } }}
                      className="group border-b transition-colors last:border-b-0 hover:bg-muted/40"
                    >
                      {row.getVisibleCells().map((cell) => (
                        <td key={cell.id} className="px-4 py-3 align-top">
                          {flexRender(
                            cell.column.columnDef.cell,
                            cell.getContext(),
                          )}
                        </td>
                      ))}
                    </motion.tr>
                  ))}
                </AnimatePresence>
              )}
            </tbody>
          </table>
        </div>

        <div className="flex flex-col gap-3 border-t border-border px-4 py-4 text-sm text-muted-foreground md:flex-row md:items-center md:justify-between">
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-2">
              <span>Rows per page</span>
              <Select
                value={String(table.getState().pagination.pageSize)}
                onValueChange={(value) => table.setPageSize(Number(value))}
              >
                <SelectTrigger className="h-9 w-[5.5rem] rounded-xl border-border bg-muted/50">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="8">8</SelectItem>
                  <SelectItem value="12">12</SelectItem>
                  <SelectItem value="20">20</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <p>
              Showing{" "}
              <span className="font-medium text-foreground">
                {table.getRowModel().rows.length}
              </span>{" "}
              of{" "}
              <span className="font-medium text-foreground">
                {filteredRows.length}
              </span>{" "}
              filtered trades
            </p>
          </div>

          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => table.previousPage()}
              disabled={!table.getCanPreviousPage()}
            >
              Previous
            </Button>
            <div className="rounded-xl border border-border bg-muted px-3 py-1.5 text-xs font-medium text-foreground">
              Page {table.getState().pagination.pageIndex + 1} of{" "}
              {table.getPageCount() || 1}
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => table.nextPage()}
              disabled={!table.getCanNextPage()}
            >
              Next
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
