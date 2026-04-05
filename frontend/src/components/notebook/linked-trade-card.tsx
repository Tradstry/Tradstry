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
          ? "border-emerald-200 bg-emerald-50/50"
          : "border-red-200 bg-red-50/50",
      )}
    >
      {onUnlink && (
        <button
          onClick={onUnlink}
          className="absolute right-2 top-2 rounded-md p-0.5 text-slate-400 opacity-0 transition-opacity hover:text-slate-600 group-hover:opacity-100"
          title="Unlink trade"
        >
          <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
        </button>
      )}

      {/* Header: symbol + status badge */}
      <div className="flex items-center gap-2">
        <span className="text-sm font-semibold text-slate-900">
          {trade.symbol}
        </span>
        <span
          className={cn(
            "rounded-full px-2 py-0.5 text-[0.6rem] font-semibold uppercase tracking-wide",
            isProfit
              ? "bg-emerald-100 text-emerald-700"
              : "bg-red-100 text-red-700",
          )}
        >
          {trade.status}
        </span>
        <span className="text-xs text-slate-400">
          {trade.tradeType}
        </span>
      </div>

      {/* Key metrics row */}
      <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs">
        <div>
          <span className="text-slate-400">P/L </span>
          <span
            className={cn(
              "font-semibold",
              isProfit ? "text-emerald-600" : "text-red-600",
            )}
          >
            {plSign}${trade.totalPl.toFixed(2)}
          </span>
          <span className="ml-1 text-slate-400">
            ({plSign}{trade.netRoi.toFixed(2)}%)
          </span>
        </div>
        <div>
          <span className="text-slate-400">Entry </span>
          <span className="font-medium text-slate-700">${trade.entryPrice.toFixed(2)}</span>
        </div>
        <div>
          <span className="text-slate-400">Exit </span>
          <span className="font-medium text-slate-700">${trade.exitPrice.toFixed(2)}</span>
        </div>
        <div>
          <span className="text-slate-400">SL </span>
          <span className="font-medium text-slate-700">${trade.stopLoss.toFixed(2)}</span>
        </div>
        <div>
          <span className="text-slate-400">R:R </span>
          <span className="font-medium text-slate-700">{trade.riskReward.toFixed(2)}</span>
        </div>
      </div>

      {/* Date + mistakes */}
      <div className="mt-1.5 flex flex-wrap gap-x-4 text-xs text-slate-400">
        <span>{trade.openDate} → {trade.closeDate}</span>
        {trade.mistakes && trade.mistakes.trim() && (
          <span className="text-red-400">Mistakes: {trade.mistakes}</span>
        )}
      </div>
    </div>
  );
}
