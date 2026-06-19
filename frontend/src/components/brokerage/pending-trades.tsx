"use client";

import { useQueryClient } from "@tanstack/react-query";
import { useActiveAccount } from "@/components/accounts";
import { MergeTradesModal } from "@/components/brokerage/merge-trades-modal";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { usePendingTrades } from "@/hooks/brokerage";
import type { PendingTrade } from "@/lib/types/brokerage";
import { cn, formatPnl } from "@/lib/utils";

function fmtDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "2-digit",
  }).format(d);
}

function fmtQty(n: number): string {
  if (!Number.isFinite(n)) return "0";
  return n % 1 === 0
    ? n.toFixed(0)
    : n.toFixed(4).replace(/0+$/, "").replace(/\.$/, "");
}

function fmtPrice(n: number | null): string {
  if (n === null || !Number.isFinite(n)) return "—";
  return `$${n.toFixed(2)}`;
}

function StatusPill({ status }: { status: "open" | "closed" }) {
  return (
    <span
      className={cn(
        "inline-flex rounded-full border px-2 py-0.5 text-[0.6rem] font-semibold uppercase tracking-wide",
        status === "open"
          ? "border-amber-300 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300"
          : "border-border bg-muted text-muted-foreground",
      )}
    >
      {status}
    </span>
  );
}

function DirectionPill({ direction }: { direction: "long" | "short" }) {
  return (
    <span
      className={cn(
        "inline-flex rounded-md px-1.5 py-0.5 text-[0.6rem] font-semibold uppercase tracking-wide",
        direction === "long"
          ? "bg-sky-100 dark:bg-sky-900/50 text-sky-700 dark:text-sky-300"
          : "bg-orange-100 text-orange-700",
      )}
    >
      {direction}
    </span>
  );
}

function PartialPill() {
  return (
    <span className="inline-flex rounded-md bg-violet-100 dark:bg-violet-900/50 px-1.5 py-0.5 text-[0.6rem] font-semibold uppercase tracking-wide text-violet-700 dark:text-violet-300">
      partially journaled
    </span>
  );
}

function PendingTradeRow({ trade }: { trade: PendingTrade }) {
  const queryClient = useQueryClient();
  const account = useActiveAccount();

  function handleSuccess() {
    // Refetch pending trades + linked tx ids so this row drops off the list.
    queryClient.invalidateQueries({
      queryKey: ["pending-trades", account?.id ?? null],
    });
    queryClient.invalidateQueries({
      queryKey: ["linked-brokerage-tx-ids", account?.id ?? null],
    });
  }

  return (
    <tr className="border-b border-border/60 hover:bg-muted/40">
      <td className="px-3 py-2.5">
        <StatusPill status={trade.status} />
      </td>
      <td className="px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span className="font-semibold tracking-wide text-foreground">
            {trade.symbol}
          </span>
          <DirectionPill direction={trade.direction} />
          {trade.isPartiallyLinked && <PartialPill />}
        </div>
      </td>
      <td className="px-3 py-2.5 text-xs tabular-nums text-muted-foreground">
        {fmtDate(trade.openDate)}
      </td>
      <td className="px-3 py-2.5 text-xs tabular-nums text-muted-foreground">
        {fmtDate(trade.closeDate)}
      </td>
      <td className="px-3 py-2.5 text-xs tabular-nums">
        {fmtQty(trade.entryUnits)}
      </td>
      <td className="px-3 py-2.5 text-xs tabular-nums">
        {fmtPrice(trade.avgEntryPrice)}
      </td>
      <td className="px-3 py-2.5 text-xs tabular-nums">
        {fmtPrice(trade.avgExitPrice)}
      </td>
      <td
        className={cn(
          "px-3 py-2.5 text-xs font-medium tabular-nums",
          trade.realizedPnl === null
            ? "text-muted-foreground"
            : trade.realizedPnl >= 0
              ? "text-emerald-600 dark:text-emerald-400"
              : "text-rose-600 dark:text-rose-400",
        )}
      >
        {trade.realizedPnl === null
          ? "—"
          : formatPnl(trade.realizedPnl, { precision: "cents" })}
      </td>
      <td className="px-3 py-2.5 text-xs tabular-nums text-muted-foreground">
        {trade.fillCount}
      </td>
      <td className="px-3 py-2.5">
        <MergeTradesModal
          prefillTransactionIds={trade.transactionIds}
          onSuccess={handleSuccess}
          trigger={
            <Button size="sm" variant="default">
              Journal
            </Button>
          }
        />
      </td>
    </tr>
  );
}

export function PendingTrades() {
  const account = useActiveAccount();
  const accountId = account?.id ?? null;
  const { data, isLoading, error } = usePendingTrades(accountId);

  if (error) {
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <div className="rounded-xl border border-rose-200 dark:border-rose-900 bg-rose-50 dark:bg-rose-950/50 p-6 text-center">
          <p className="font-medium text-rose-700 dark:text-rose-300">
            Failed to load pending trades
          </p>
          <p className="mt-1 text-xs text-rose-600 dark:text-rose-400">{error.message}</p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex flex-col gap-2 p-4 md:p-6">
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
      </div>
    );
  }

  const trades = data ?? [];

  if (trades.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-12">
        <div className="text-center">
          <p className="text-sm font-medium text-foreground">
            No pending trades
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            All your trades are journaled. New trades will appear here after the
            next sync.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col overflow-hidden p-4 md:p-6">
      <div className="overflow-x-auto rounded-xl border bg-background">
        <table className="min-w-full">
          <thead className="bg-muted/40 text-[0.65rem] font-medium uppercase tracking-wide text-muted-foreground">
            <tr>
              <th className="px-3 py-2 text-left">Status</th>
              <th className="px-3 py-2 text-left">Symbol</th>
              <th className="px-3 py-2 text-left">Open</th>
              <th className="px-3 py-2 text-left">Close</th>
              <th className="px-3 py-2 text-left">Qty</th>
              <th className="px-3 py-2 text-left">Avg entry</th>
              <th className="px-3 py-2 text-left">Avg exit</th>
              <th className="px-3 py-2 text-left">Realized P&L</th>
              <th className="px-3 py-2 text-left">Fills</th>
              <th className="px-3 py-2 text-left"></th>
            </tr>
          </thead>
          <tbody>
            {trades.map((trade) => (
              <PendingTradeRow key={trade.id} trade={trade} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
