"use client";

import { Alert02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useQueryClient } from "@tanstack/react-query";
import { MergeTradesModal } from "@tradstry/app-ui/components/brokerage/merge-trades-modal";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { Skeleton } from "@tradstry/app-ui/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@tradstry/app-ui/components/ui/tooltip";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import { usePendingTrades } from "@tradstry/app-ui/hooks/brokerage";
import { useTradeReviewInbox } from "@tradstry/app-ui/hooks/position-calculator";
import type { PendingTrade } from "@tradstry/app-ui/lib/types/brokerage";
import type { TradeReviewInboxItem } from "@tradstry/app-ui/lib/types/position-calculator";
import { cn, formatPnl } from "@tradstry/app-ui/lib/utils";

const PENDING_SKELETON_ROWS = [
  "pending-skeleton-1",
  "pending-skeleton-2",
  "pending-skeleton-3",
  "pending-skeleton-4",
  "pending-skeleton-5",
  "pending-skeleton-6",
  "pending-skeleton-7",
  "pending-skeleton-8",
];

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
        "inline-flex rounded-full border px-2 py-0.5 text-[0.625rem] font-semibold uppercase tracking-[0.06em]",
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

function OptionPill({ kind }: { kind: string | null }) {
  const label = kind === "CALL" ? "Call" : kind === "PUT" ? "Put" : "Option";
  return (
    <span className="inline-flex rounded-md bg-indigo-100 dark:bg-indigo-900/50 px-1.5 py-0.5 text-[0.6rem] font-semibold uppercase tracking-wide text-indigo-700 dark:text-indigo-300">
      {label}
    </span>
  );
}

function GroupingWarning({ reason }: { reason: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={`Grouping needs review. ${reason}`}
          className="inline-flex h-5 items-center gap-1 rounded-full border border-amber-300/80 bg-amber-50 px-1.5 text-[0.625rem] font-medium text-amber-700 transition-colors hover:border-amber-400 hover:bg-amber-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400/50 focus-visible:ring-offset-1 dark:border-amber-800 dark:bg-amber-950/50 dark:text-amber-300 dark:hover:border-amber-700 dark:hover:bg-amber-950"
        >
          <HugeiconsIcon
            icon={Alert02Icon}
            className="size-3 shrink-0"
            strokeWidth={2}
          />
          Grouping review
        </button>
      </TooltipTrigger>
      <TooltipContent
        side="top"
        align="start"
        className="max-w-64 flex-col items-start gap-1 text-left"
      >
        <span className="font-medium">Grouping needs review</span>
        <span className="text-background/75">{reason}</span>
      </TooltipContent>
    </Tooltip>
  );
}

function PlanStatus({ review }: { review: TradeReviewInboxItem | undefined }) {
  if (review?.confirmedPlanId) {
    return (
      <span className="inline-flex rounded-full border border-emerald-200 bg-emerald-50 px-2 py-0.5 text-[0.625rem] font-medium text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-300">
        Plan matched
      </span>
    );
  }
  let hasSuggestion = false;
  try {
    hasSuggestion = JSON.parse(review?.suggestionsJson ?? "[]").length > 0;
  } catch {
    hasSuggestion = false;
  }
  return hasSuggestion ? (
    <span className="inline-flex rounded-full border border-sky-200 bg-sky-50 px-2 py-0.5 text-[0.625rem] font-medium text-sky-700 dark:border-sky-900 dark:bg-sky-950/40 dark:text-sky-300">
      Plan suggested
    </span>
  ) : (
    <span className="text-[0.6875rem] text-muted-foreground">No plan</span>
  );
}

function PendingTradeRow({
  trade,
  review,
  onAdjustFills,
}: {
  trade: PendingTrade;
  review: TradeReviewInboxItem | undefined;
  onAdjustFills: (
    episodeId: string,
    transactionIds: string[],
    symbol: string,
  ) => void;
}) {
  const queryClient = useQueryClient();
  const account = useActiveWorkspace();

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
    <tr
      className={cn(
        "group transition-colors hover:bg-muted/35",
        trade.blockReason &&
          "bg-amber-50/20 hover:bg-amber-50/45 dark:bg-amber-950/5 dark:hover:bg-amber-950/15",
      )}
    >
      <td className="border-b border-border/60 px-3 py-3">
        <StatusPill status={trade.status} />
      </td>
      <td className="border-b border-border/60 px-3 py-3">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs font-semibold tracking-wide text-foreground">
            {trade.isOption ? (trade.symbolName ?? trade.symbol) : trade.symbol}
          </span>
          <DirectionPill direction={trade.direction} />
          {trade.isOption && <OptionPill kind={trade.optionKind} />}
          {trade.isPartiallyLinked && <PartialPill />}
          {trade.isManuallyGrouped ? (
            <span className="inline-flex rounded-full border border-sky-200 bg-sky-50 px-2 py-0.5 text-[0.625rem] font-medium text-sky-700 dark:border-sky-900 dark:bg-sky-950/40 dark:text-sky-300">
              Manually grouped
            </span>
          ) : null}
          {trade.blockReason ? (
            <GroupingWarning reason={trade.blockReason} />
          ) : null}
        </div>
      </td>
      <td className="whitespace-nowrap border-b border-border/60 px-3 py-3 text-xs tabular-nums text-muted-foreground">
        {fmtDate(trade.openDate)}
      </td>
      <td className="whitespace-nowrap border-b border-border/60 px-3 py-3 text-xs tabular-nums text-muted-foreground">
        {fmtDate(trade.closeDate)}
      </td>
      <td className="border-b border-border/60 px-3 py-3 text-right font-mono text-xs tabular-nums">
        {fmtQty(trade.entryUnits)}
      </td>
      <td className="border-b border-border/60 px-3 py-3 text-right font-mono text-xs tabular-nums">
        {fmtPrice(trade.avgEntryPrice)}
      </td>
      <td className="border-b border-border/60 px-3 py-3 text-right font-mono text-xs tabular-nums">
        {fmtPrice(trade.avgExitPrice)}
      </td>
      <td
        className={cn(
          "border-b border-border/60 px-3 py-3 text-right font-mono text-xs font-semibold tabular-nums",
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
      <td className="border-b border-border/60 px-3 py-3 text-right font-mono text-xs tabular-nums text-muted-foreground">
        {trade.fillCount}
      </td>
      <td className="border-b border-border/60 px-3 py-3">
        <PlanStatus review={review} />
      </td>
      <td className="border-b border-border/60 px-3 py-3 text-right">
        {trade.status === "open" ? (
          <Button size="sm" variant="outline" disabled>
            Still open
          </Button>
        ) : trade.requiresManualGrouping || trade.isPartiallyLinked ? (
          <Button
            size="sm"
            variant="outline"
            onClick={() =>
              onAdjustFills(trade.episodeId, trade.transactionIds, trade.symbol)
            }
          >
            Adjust fills
          </Button>
        ) : (
          <MergeTradesModal
            episodeId={trade.episodeId}
            prefillTransactionIds={trade.transactionIds}
            isManuallyGrouped={trade.isManuallyGrouped}
            onEditGrouping={() =>
              onAdjustFills(trade.episodeId, trade.transactionIds, trade.symbol)
            }
            onSuccess={handleSuccess}
            trigger={<Button size="sm">Review</Button>}
          />
        )}
      </td>
    </tr>
  );
}

export function PendingTrades({
  onAdjustFills,
}: {
  onAdjustFills: (
    episodeId: string,
    transactionIds: string[],
    symbol: string,
  ) => void;
}) {
  const account = useActiveWorkspace();
  const workspaceId = account?.id ?? null;
  const { data, isLoading, error } = usePendingTrades(workspaceId);
  const reviewInbox = useTradeReviewInbox(!!workspaceId);

  if (error) {
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <div className="rounded-xl border border-rose-200 dark:border-rose-900 bg-rose-50 dark:bg-rose-950/50 p-6 text-center">
          <p className="font-medium text-rose-700 dark:text-rose-300">
            Failed to load pending trades
          </p>
          <p className="mt-1 text-xs text-rose-600 dark:text-rose-400">
            {error.message}
          </p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex min-h-0 flex-1 flex-col p-3 md:p-5 xl:p-6">
        <div className="flex flex-1 flex-col overflow-hidden rounded-2xl border bg-background">
          <div className="border-b px-4 py-3">
            <Skeleton className="h-8 w-56" />
          </div>
          {PENDING_SKELETON_ROWS.map((row) => (
            <div key={row} className="border-b px-4 py-4 last:border-0">
              <Skeleton className="h-4 w-full" />
            </div>
          ))}
        </div>
      </div>
    );
  }

  const trades = data ?? [];

  if (trades.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-12">
        <div className="max-w-sm rounded-2xl border bg-background px-8 py-10 text-center shadow-sm">
          <div className="mx-auto mb-4 flex size-10 items-center justify-center rounded-full border bg-muted/40 text-sm font-semibold">
            0
          </div>
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
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden p-3 md:p-5 xl:p-6">
      <section
        aria-label="Pending trades"
        className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border/80 bg-background shadow-[0_1px_2px_rgb(0_0_0/0.04),0_10px_30px_rgb(0_0_0/0.025)]"
      >
        <header className="shrink-0 border-b bg-muted/15 px-4 py-3">
          <h2 className="text-sm font-semibold tracking-tight">
            Trades ready to journal
          </h2>
          <p className="text-[0.6875rem] text-muted-foreground">
            {trades.length.toLocaleString()} broker positions awaiting review
          </p>
        </header>
        <div className="min-h-0 flex-1 overflow-auto overscroll-contain">
          <table className="w-full min-w-[64rem] border-separate border-spacing-0">
            <thead className="sticky top-0 z-10 bg-background/95 text-[0.625rem] font-semibold uppercase tracking-[0.08em] text-muted-foreground shadow-[0_1px_0_var(--border)] backdrop-blur-sm">
              <tr>
                <th className="h-10 px-3 py-2 text-left">Status</th>
                <th className="h-10 min-w-64 px-3 py-2 text-left">Security</th>
                <th className="h-10 px-3 py-2 text-left">Opened</th>
                <th className="h-10 px-3 py-2 text-left">Closed</th>
                <th className="h-10 px-3 py-2 text-right">Qty</th>
                <th className="h-10 px-3 py-2 text-right">Avg entry</th>
                <th className="h-10 px-3 py-2 text-right">Avg exit</th>
                <th className="h-10 px-3 py-2 text-right">Realized P&amp;L</th>
                <th className="h-10 px-3 py-2 text-right">Fills</th>
                <th className="h-10 px-3 py-2 text-left">Plan</th>
                <th className="h-10 px-3 py-2 text-right">
                  <span className="sr-only">Actions</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {trades.map((trade) => (
                <PendingTradeRow
                  key={trade.id}
                  trade={trade}
                  review={(reviewInbox.data ?? []).find(
                    (item) => item.episodeId === trade.episodeId,
                  )}
                  onAdjustFills={onAdjustFills}
                />
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
