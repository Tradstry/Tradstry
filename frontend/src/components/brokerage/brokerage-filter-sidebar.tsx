"use client";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { TransactionFilters } from "@/lib/types/brokerage";

// ---------------------------------------------------------------------------
// Type filter groups
// ---------------------------------------------------------------------------

const TYPE_GROUPS = [
  { label: "Trading", types: ["BUY", "SELL"], color: "bg-emerald-100 text-emerald-800 border-emerald-300" },
  { label: "Income", types: ["DIVIDEND", "STOCK_DIVIDEND", "INTEREST", "REI"], color: "bg-indigo-100 text-indigo-800 border-indigo-300" },
  { label: "Options", types: ["OPTIONEXPIRATION", "OPTIONASSIGNMENT", "OPTIONEXERCISE"], color: "bg-violet-100 text-violet-800 border-violet-300" },
  { label: "Transfers", types: ["TRANSFER", "CONTRIBUTION", "WITHDRAWAL", "EXTERNAL_ASSET_TRANSFER_IN", "EXTERNAL_ASSET_TRANSFER_OUT"], color: "bg-amber-100 text-amber-800 border-amber-300" },
  { label: "Other", types: ["FEE", "TAX", "SPLIT", "ADJUSTMENT"], color: "bg-slate-100 text-slate-700 border-slate-300" },
] as const;

// ---------------------------------------------------------------------------
// Sync status indicator
// ---------------------------------------------------------------------------

type SyncState = "idle" | "syncing" | "synced" | "error";

function SyncStatus({
  state,
  lastSyncTime,
  onRetry,
}: {
  state: SyncState;
  lastSyncTime: string | null;
  onRetry: () => void;
}) {
  if (state === "syncing") {
    return (
      <div className="rounded-lg border border-amber-200 bg-amber-50 p-2.5 text-[0.65rem]">
        <span className="font-semibold text-amber-700">Syncing...</span>
        <p className="mt-0.5 text-muted-foreground">Fetching transactions</p>
      </div>
    );
  }
  if (state === "error") {
    return (
      <div className="rounded-lg border border-rose-200 bg-rose-50 p-2.5 text-[0.65rem]">
        <span className="font-semibold text-rose-700">Sync failed</span>
        <button onClick={onRetry} className="mt-1 block text-rose-600 underline underline-offset-2">
          Retry
        </button>
      </div>
    );
  }
  if (state === "synced" && lastSyncTime) {
    const ago = getRelativeTime(lastSyncTime);
    return (
      <div className="rounded-lg border border-emerald-200 bg-emerald-50 p-2.5 text-[0.65rem]">
        <span className="font-semibold text-emerald-700">Synced</span>
        <p className="mt-0.5 text-muted-foreground">Last: {ago}</p>
      </div>
    );
  }
  return null;
}

function getRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ago`;
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface BrokerageFilterSidebarProps {
  filters: TransactionFilters;
  onFiltersChange: (filters: TransactionFilters) => void;
  symbolSearch: string;
  onSymbolSearchChange: (v: string) => void;
  descriptionSearch: string;
  onDescriptionSearchChange: (v: string) => void;
  syncState: SyncState;
  lastSyncTime: string | null;
  onRetrySync: () => void;
  onDisconnect?: () => void;
  isDisconnecting?: boolean;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function BrokerageFilterSidebar({
  filters,
  onFiltersChange,
  symbolSearch,
  onSymbolSearchChange,
  descriptionSearch,
  onDescriptionSearchChange,
  syncState,
  lastSyncTime,
  onRetrySync,
  onDisconnect,
  isDisconnecting,
}: BrokerageFilterSidebarProps) {
  const activeType = filters.transactionType ?? null;

  function setType(type: string | null) {
    onFiltersChange({ ...filters, transactionType: type ?? undefined, offset: 0 });
  }

  function clearAll() {
    onFiltersChange({ offset: 0, limit: filters.limit });
    onSymbolSearchChange("");
    onDescriptionSearchChange("");
  }

  return (
    <aside className="flex w-[200px] shrink-0 flex-col gap-4 border-r bg-muted/30 p-4">
      {/* Sync status */}
      <SyncStatus state={syncState} lastSyncTime={lastSyncTime} onRetry={onRetrySync} />

      {/* Symbol search */}
      <div>
        <label className="mb-1 block text-[0.6rem] font-semibold uppercase tracking-[0.15em] text-muted-foreground">
          Symbol
        </label>
        <Input
          placeholder="Search ticker..."
          value={symbolSearch}
          onChange={(e) => onSymbolSearchChange(e.target.value)}
          className="h-7"
        />
        {symbolSearch && (
          <p className="mt-0.5 text-[0.55rem] text-muted-foreground">Searching all trades</p>
        )}
      </div>

      {/* Type filter */}
      <div>
        <label className="mb-1 block text-[0.6rem] font-semibold uppercase tracking-[0.15em] text-muted-foreground">
          Type
        </label>
        <div className="flex flex-wrap gap-1">
          <button
            onClick={() => setType(null)}
            className={`rounded-full border px-2 py-0.5 text-[0.6rem] font-medium transition-colors ${
              !activeType
                ? "border-foreground bg-foreground text-background"
                : "border-border bg-background text-muted-foreground hover:bg-muted"
            }`}
          >
            All
          </button>
          {TYPE_GROUPS.map((group) =>
            group.types.map((type) => (
              <button
                key={type}
                onClick={() => setType(activeType === type ? null : type)}
                className={`rounded-full border px-2 py-0.5 text-[0.6rem] font-medium transition-colors ${
                  activeType === type ? group.color : "border-border bg-background text-muted-foreground hover:bg-muted"
                }`}
              >
                {type}
              </button>
            ))
          )}
        </div>
      </div>

      {/* Date range */}
      <div>
        <label className="mb-1 block text-[0.6rem] font-semibold uppercase tracking-[0.15em] text-muted-foreground">
          Date Range
        </label>
        <Input
          type="date"
          placeholder="From"
          value={filters.startDate ?? ""}
          onChange={(e) => onFiltersChange({ ...filters, startDate: e.target.value || undefined, offset: 0 })}
          className="mb-1 h-7"
        />
        <Input
          type="date"
          placeholder="To"
          value={filters.endDate ?? ""}
          onChange={(e) => onFiltersChange({ ...filters, endDate: e.target.value || undefined, offset: 0 })}
          className="h-7"
        />
      </div>

      {/* Description search */}
      <div>
        <label className="mb-1 block text-[0.6rem] font-semibold uppercase tracking-[0.15em] text-muted-foreground">
          Description
        </label>
        <Input
          placeholder="Search..."
          value={descriptionSearch}
          onChange={(e) => onDescriptionSearchChange(e.target.value)}
          className="h-7"
        />
        {descriptionSearch && (
          <p className="mt-0.5 text-[0.55rem] text-muted-foreground">Filtering current page</p>
        )}
      </div>

      {/* Clear all */}
      <button
        onClick={clearAll}
        className="text-center text-[0.65rem] text-primary hover:underline"
      >
        Clear all filters
      </button>

      {/* Re-sync & Disconnect */}
      <div className="mt-auto flex flex-col gap-2 border-t pt-3">
        <Button
          variant="outline"
          size="xs"
          className="w-full"
          onClick={onRetrySync}
          disabled={syncState === "syncing"}
        >
          Re-sync now
        </Button>
        {onDisconnect && (
          <Button
            variant="outline"
            size="xs"
            className="w-full text-destructive hover:bg-destructive/10"
            onClick={onDisconnect}
            disabled={isDisconnecting}
          >
            {isDisconnecting ? "Disconnecting..." : "Disconnect brokerage"}
          </Button>
        )}
      </div>
    </aside>
  );
}
