import { useMemo, useState } from "react";
import {
  Cell,
  Column,
  Row,
  Table,
  TableBody,
  TableHeader,
  type SortDescriptor,
} from "react-aria-components";

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

const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });

const headerClass =
  "cursor-pointer border-b border-zinc-200 px-3 py-2 text-left text-xs font-medium uppercase tracking-wide text-zinc-500 outline-none data-focus-visible:outline-2 data-focus-visible:-outline-offset-2 data-focus-visible:outline-blue-500 dark:border-zinc-800 dark:text-zinc-400";
const cellClass =
  "border-b border-zinc-100 px-3 py-2 text-sm text-zinc-800 outline-none data-focus-visible:outline-2 data-focus-visible:-outline-offset-2 data-focus-visible:outline-blue-500 dark:border-zinc-900 dark:text-zinc-200";
const numCellClass = `${cellClass} text-right tabular-nums`;

function SortArrow({ direction }: { direction?: "ascending" | "descending" }) {
  if (!direction) return null;
  return (
    <svg
      viewBox="0 0 12 12"
      aria-hidden="true"
      className={`h-3 w-3 shrink-0 transition-transform duration-150 ${direction === "descending" ? "rotate-180" : ""}`}
    >
      <path d="M6 3l4 5H2z" fill="currentColor" />
    </svg>
  );
}

export default function TradesTable() {
  const [sortDescriptor, setSortDescriptor] = useState<SortDescriptor>({
    column: "date",
    direction: "descending",
  });

  const sortedTrades = useMemo(() => {
    const column = sortDescriptor.column as keyof Trade;
    return [...TRADES].sort((a, b) => {
      const av = a[column];
      const bv = b[column];
      const cmp =
        typeof av === "number" && typeof bv === "number"
          ? av - bv
          : String(av).localeCompare(String(bv));
      return sortDescriptor.direction === "descending" ? -cmp : cmp;
    });
  }, [sortDescriptor]);

  return (
    <div className="overflow-auto overscroll-none rounded-lg border border-zinc-200/80 bg-white/85 backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <Table
        aria-label="Trades"
        selectionMode="single"
        sortDescriptor={sortDescriptor}
        onSortChange={setSortDescriptor}
        className="w-full border-separate border-spacing-0"
      >
        <TableHeader>
          <Column id="date" isRowHeader allowsSorting className={headerClass}>
            {({ sortDirection }) => (
              <span className="flex items-center gap-1">
                Date <SortArrow direction={sortDirection} />
              </span>
            )}
          </Column>
          <Column id="symbol" allowsSorting className={headerClass}>
            {({ sortDirection }) => (
              <span className="flex items-center gap-1">
                Symbol <SortArrow direction={sortDirection} />
              </span>
            )}
          </Column>
          <Column id="side" className={headerClass}>
            Side
          </Column>
          <Column id="qty" allowsSorting className={`${headerClass} text-right`}>
            {({ sortDirection }) => (
              <span className="flex items-center justify-end gap-1">
                Qty <SortArrow direction={sortDirection} />
              </span>
            )}
          </Column>
          <Column id="entry" className={`${headerClass} text-right`}>
            Entry
          </Column>
          <Column id="exit" className={`${headerClass} text-right`}>
            Exit
          </Column>
          <Column id="pnl" allowsSorting className={`${headerClass} text-right`}>
            {({ sortDirection }) => (
              <span className="flex items-center justify-end gap-1">
                P&amp;L <SortArrow direction={sortDirection} />
              </span>
            )}
          </Column>
        </TableHeader>
        <TableBody items={sortedTrades}>
          {(trade) => (
            <Row
              className="cursor-default outline-none data-hovered:bg-zinc-50 data-selected:bg-blue-50 data-focus-visible:outline-2 data-focus-visible:-outline-offset-2 data-focus-visible:outline-blue-500 dark:data-hovered:bg-zinc-900 dark:data-selected:bg-blue-950/40"
            >
              <Cell className={cellClass}>{trade.date}</Cell>
              <Cell className={`${cellClass} font-medium`}>{trade.symbol}</Cell>
              <Cell className={cellClass}>{trade.side}</Cell>
              <Cell className={numCellClass}>{trade.qty}</Cell>
              <Cell className={numCellClass}>{usd.format(trade.entry)}</Cell>
              <Cell className={numCellClass}>{usd.format(trade.exit)}</Cell>
              <Cell
                className={`${numCellClass} font-medium ${
                  trade.pnl >= 0
                    ? "text-emerald-600 dark:text-emerald-400"
                    : "text-red-600 dark:text-red-400"
                }`}
              >
                {trade.pnl >= 0 ? "+" : ""}
                {usd.format(trade.pnl)}
              </Cell>
            </Row>
          )}
        </TableBody>
      </Table>
    </div>
  );
}
