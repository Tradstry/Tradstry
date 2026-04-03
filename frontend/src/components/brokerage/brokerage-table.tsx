"use client";

import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  type SortingState,
  useReactTable,
} from "@tanstack/react-table";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import type { BrokerageTransaction } from "@/lib/types/brokerage";

// ---------------------------------------------------------------------------
// Type badge colors
// ---------------------------------------------------------------------------

const TYPE_COLORS: Record<string, string> = {
  BUY: "border-emerald-200 bg-emerald-50 text-emerald-700",
  SELL: "border-rose-200 bg-rose-50 text-rose-700",
  DIVIDEND: "border-indigo-200 bg-indigo-50 text-indigo-700",
  STOCK_DIVIDEND: "border-indigo-200 bg-indigo-50 text-indigo-700",
  INTEREST: "border-indigo-200 bg-indigo-50 text-indigo-700",
  REI: "border-indigo-200 bg-indigo-50 text-indigo-700",
  OPTIONEXPIRATION: "border-violet-200 bg-violet-50 text-violet-700",
  OPTIONASSIGNMENT: "border-violet-200 bg-violet-50 text-violet-700",
  OPTIONEXERCISE: "border-violet-200 bg-violet-50 text-violet-700",
  TRANSFER: "border-amber-200 bg-amber-50 text-amber-700",
  CONTRIBUTION: "border-amber-200 bg-amber-50 text-amber-700",
  WITHDRAWAL: "border-amber-200 bg-amber-50 text-amber-700",
  EXTERNAL_ASSET_TRANSFER_IN: "border-amber-200 bg-amber-50 text-amber-700",
  EXTERNAL_ASSET_TRANSFER_OUT: "border-amber-200 bg-amber-50 text-amber-700",
};
const DEFAULT_TYPE_COLOR = "border-slate-200 bg-slate-50 text-slate-700";

function TypeBadge({ type }: { type: string }) {
  const cls = TYPE_COLORS[type] ?? DEFAULT_TYPE_COLOR;
  return (
    <span className={`inline-flex rounded-full border px-2.5 py-0.5 text-[0.65rem] font-medium ${cls}`}>
      {type}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

function fmtDate(iso: string | null): string {
  if (!iso) return "\u2014";
  return new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" }).format(new Date(iso));
}

function fmtCurrency(value: number | null, currency = "USD"): string {
  if (value == null) return "\u2014";
  return new Intl.NumberFormat("en-US", { style: "currency", currency, minimumFractionDigits: 2 }).format(value);
}

function amountClasses(value: number | null): string {
  if (value == null) return "text-muted-foreground";
  return value < 0 ? "text-rose-600 font-medium" : "text-emerald-600 font-medium";
}

// ---------------------------------------------------------------------------
// Columns
// ---------------------------------------------------------------------------

const columns: ColumnDef<BrokerageTransaction>[] = [
  {
    accessorKey: "tradeDate",
    header: "Date",
    cell: ({ row }) => (
      <span className="text-muted-foreground">{fmtDate(row.original.tradeDate)}</span>
    ),
  },
  {
    accessorKey: "symbol",
    header: "Symbol",
    cell: ({ row }) => {
      const { symbol, symbolDescription } = row.original;
      if (!symbol) return <span className="text-muted-foreground">{"\u2014"}</span>;
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
      return v !== 0 ? v.toLocaleString() : "\u2014";
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

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface BrokerageTableProps {
  transactions: BrokerageTransaction[];
  total: number;
  page: number;
  pageSize: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: number) => void;
  isLoading: boolean;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function BrokerageTable({
  transactions,
  total,
  page,
  pageSize,
  onPageChange,
  onPageSizeChange,
  isLoading,
}: BrokerageTableProps) {
  const [sorting, setSorting] = useState<SortingState>([
    { id: "tradeDate", desc: true },
  ]);

  const table = useReactTable({
    data: transactions,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const totalPages = Math.ceil(total / pageSize);

  if (isLoading) return <BrokerageTableLoading />;

  return (
    <div className="flex flex-1 flex-col gap-3">
      {/* Header row */}
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted-foreground">
          {total.toLocaleString()} transactions
        </span>
        <Select
          value={String(pageSize)}
          onValueChange={(v) => {
            onPageSizeChange(Number(v));
            onPageChange(0);
          }}
        >
          <SelectTrigger className="w-20 h-7">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="20">20</SelectItem>
            <SelectItem value="50">50</SelectItem>
            <SelectItem value="100">100</SelectItem>
          </SelectContent>
        </Select>
      </div>

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
            {table.getRowModel().rows.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="px-3 py-12 text-center text-muted-foreground">
                  No transactions found.
                </td>
              </tr>
            ) : (
              table.getRowModel().rows.map((row) => (
                <tr key={row.id} className="border-b last:border-0">
                  {row.getVisibleCells().map((cell) => (
                    <td key={cell.id} className="px-3 py-2">
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-center gap-1.5 text-xs">
          <Button
            variant="outline"
            size="xs"
            disabled={page === 0}
            onClick={() => onPageChange(page - 1)}
          >
            Previous
          </Button>
          <span className="px-2 text-muted-foreground">
            Page {page + 1} of {totalPages}
          </span>
          <Button
            variant="outline"
            size="xs"
            disabled={page >= totalPages - 1}
            onClick={() => onPageChange(page + 1)}
          >
            Next
          </Button>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Loading skeleton
// ---------------------------------------------------------------------------

function BrokerageTableLoading() {
  return (
    <div className="flex flex-1 flex-col gap-3">
      <div className="flex items-center justify-between">
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-7 w-20" />
      </div>
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
    </div>
  );
}
