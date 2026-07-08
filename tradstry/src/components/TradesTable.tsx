import { type ComponentProps, useState } from "react";
import {
  type ColumnDef,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { cn } from "@/lib/utils";

type Trade = {
  id: number;
  date: string;
  symbol: string;
  side: "Long" | "Short";
  qty: number;
  entry: number;
  exit: number;
  pnl: number;
};

const TRADES: Trade[] = [
  { id: 1, date: "2026-07-01", symbol: "NVDA", side: "Long", qty: 100, entry: 128.4, exit: 134.2, pnl: 580.0 },
  { id: 2, date: "2026-07-01", symbol: "TSLA", side: "Short", qty: 50, entry: 312.8, exit: 305.1, pnl: 385.0 },
  { id: 3, date: "2026-07-02", symbol: "AAPL", side: "Long", qty: 200, entry: 214.6, exit: 212.9, pnl: -340.0 },
  { id: 4, date: "2026-07-02", symbol: "AMD", side: "Long", qty: 150, entry: 162.3, exit: 168.75, pnl: 967.5 },
  { id: 5, date: "2026-07-03", symbol: "META", side: "Short", qty: 40, entry: 598.2, exit: 604.5, pnl: -252.0 },
  { id: 6, date: "2026-07-03", symbol: "SPY", side: "Long", qty: 75, entry: 552.1, exit: 555.8, pnl: 277.5 },
  { id: 7, date: "2026-07-05", symbol: "QQQ", side: "Short", qty: 60, entry: 489.4, exit: 483.2, pnl: 372.0 },
  { id: 8, date: "2026-07-05", symbol: "MSFT", side: "Long", qty: 80, entry: 448.9, exit: 445.3, pnl: -288.0 },
];

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});

function SortableHeader({
  label,
  align = "left",
  sortDirection,
  onClick,
}: {
  label: string;
  align?: "left" | "right";
  sortDirection: false | "asc" | "desc";
  onClick?: ComponentProps<"button">["onClick"];
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1 font-medium",
        align === "right" && "justify-end",
      )}
    >
      <span>{label}</span>
      {sortDirection === "asc" ? <span>↑</span> : null}
      {sortDirection === "desc" ? <span>↓</span> : null}
    </button>
  );
}

const columns: ColumnDef<Trade>[] = [
  {
    accessorKey: "date",
    header: ({ column }) => (
      <SortableHeader
        label="Date"
        sortDirection={column.getIsSorted()}
        onClick={column.getToggleSortingHandler()}
      />
    ),
    cell: ({ row }) => row.original.date,
  },
  {
    accessorKey: "symbol",
    header: ({ column }) => (
      <SortableHeader
        label="Symbol"
        sortDirection={column.getIsSorted()}
        onClick={column.getToggleSortingHandler()}
      />
    ),
    cell: ({ row }) => (
      <span className="font-medium text-foreground">{row.original.symbol}</span>
    ),
  },
  { accessorKey: "side", header: "Side", enableSorting: false },
  {
    accessorKey: "qty",
    header: ({ column }) => (
      <SortableHeader
        label="Qty"
        align="right"
        sortDirection={column.getIsSorted()}
        onClick={column.getToggleSortingHandler()}
      />
    ),
    cell: ({ row }) => row.original.qty,
    meta: { align: "right" },
  },
  {
    accessorKey: "entry",
    header: "Entry",
    enableSorting: false,
    cell: ({ row }) => usd.format(row.original.entry),
    meta: { align: "right" },
  },
  {
    accessorKey: "exit",
    header: "Exit",
    enableSorting: false,
    cell: ({ row }) => usd.format(row.original.exit),
    meta: { align: "right" },
  },
  {
    accessorKey: "pnl",
    header: ({ column }) => (
      <SortableHeader
        label="P/L"
        align="right"
        sortDirection={column.getIsSorted()}
        onClick={column.getToggleSortingHandler()}
      />
    ),
    cell: ({ row }) => (
      <span
        className={cn(
          "font-medium",
          row.original.pnl >= 0
            ? "text-emerald-600 dark:text-emerald-400"
            : "text-rose-600 dark:text-rose-400",
        )}
      >
        {row.original.pnl >= 0 ? "+" : ""}
        {usd.format(row.original.pnl)}
      </span>
    ),
    meta: { align: "right" },
  },
];

export default function TradesTable() {
  const [sorting, setSorting] = useState<SortingState>([
    { id: "date", desc: true },
  ]);

  const table = useReactTable({
    data: TRADES,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <div className="overflow-auto overscroll-none rounded-lg border bg-background">
      <table className="min-w-full text-sm">
        <thead>
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id} className="border-b">
              {headerGroup.headers.map((header) => {
                const align =
                  (header.column.columnDef.meta as { align?: string })?.align ??
                  "left";
                return (
                  <th
                    key={header.id}
                    className={cn(
                      "px-3 py-2 text-[0.68rem] font-semibold uppercase tracking-[0.16em] text-muted-foreground",
                      align === "right" ? "text-right" : "text-left",
                    )}
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )}
                  </th>
                );
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr
              key={row.id}
              className="border-b transition-colors last:border-b-0 hover:bg-muted/40"
            >
              {row.getVisibleCells().map((cell) => {
                const align =
                  (cell.column.columnDef.meta as { align?: string })?.align ??
                  "left";
                return (
                  <td
                    key={cell.id}
                    className={cn(
                      "whitespace-nowrap px-3 py-2 text-foreground",
                      align === "right" && "text-right tabular-nums",
                    )}
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
