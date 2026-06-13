"use client";

import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import type { JournalEntry } from "@/lib/types/journal";
import { cn } from "@/lib/utils";

export function LinkedTradeCard({
  trade,
  onUnlink,
}: {
  trade: JournalEntry;
  onUnlink?: () => void;
}) {
  const isProfit = trade.status === "profit";
  const plSign = trade.totalPl >= 0 ? "+" : "";

  return (
    <div
      className={cn(
        "group relative rounded-xl border px-4 py-3",
        isProfit
          ? "border-emerald-200 bg-emerald-50/50 dark:border-emerald-900 dark:bg-emerald-950/40"
          : "border-red-200 bg-red-50/50 dark:border-red-900 dark:bg-red-950/40",
      )}
    >
      {onUnlink && (
        <button
          onClick={onUnlink}
          className="absolute right-2 top-2 rounded-md p-0.5 text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100"
          title="Unlink trade"
        >
          <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
        </button>
      )}

      {/* Header: symbol + status badge */}
      <div className="flex items-center gap-2">
        <span className="text-sm font-semibold text-foreground">
          {trade.symbol}
        </span>
        <span
          className={cn(
            "rounded-full px-2 py-0.5 text-[0.6rem] font-semibold uppercase tracking-wide",
            isProfit
              ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/50 dark:text-emerald-300"
              : "bg-red-100 text-red-700 dark:bg-red-900/50 dark:text-red-300",
          )}
        >
          {trade.status}
        </span>
        <span className="text-xs text-muted-foreground">{trade.tradeType}</span>
      </div>

      {/* Key metrics row */}
      <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs">
        <div>
          <span className="text-muted-foreground">P/L </span>
          <span
            className={cn(
              "font-semibold",
              isProfit
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-red-600 dark:text-red-400",
            )}
          >
            {plSign}${trade.totalPl.toFixed(2)}
          </span>
          <span className="ml-1 text-muted-foreground">
            ({plSign}
            {trade.netRoi.toFixed(2)}%)
          </span>
        </div>
        <div>
          <span className="text-muted-foreground">Entry </span>
          <span className="font-medium text-foreground">
            ${trade.entryPrice.toFixed(2)}
          </span>
        </div>
        <div>
          <span className="text-muted-foreground">Exit </span>
          <span className="font-medium text-foreground">
            ${trade.exitPrice.toFixed(2)}
          </span>
        </div>
        <div>
          <span className="text-muted-foreground">SL </span>
          <span className="font-medium text-foreground">
            ${trade.stopLoss.toFixed(2)}
          </span>
        </div>
        <div>
          <span className="text-muted-foreground">R:R </span>
          <span className="font-medium text-foreground">
            {trade.riskReward.toFixed(2)}
          </span>
        </div>
      </div>

      {/* Date + mistakes */}
      <div className="mt-1.5 flex flex-wrap gap-x-4 text-xs text-muted-foreground">
        <span>
          {trade.openDate} → {trade.closeDate}
        </span>
        {trade.mistakes && trade.mistakes.trim() && (
          <span className="text-red-400">Mistakes: {trade.mistakes}</span>
        )}
      </div>
    </div>
  );
}
