# Brokerage UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the brokerage transaction history page with sidebar filters, auto-sync, and empty state.

**Architecture:** Sidebar-filter layout with server-side pagination/type/date filtering and client-side symbol/description search. Auto-syncs on page load via mutation with 5-minute staleness window. Three states: empty (no SnapTrade linked), loading (syncing), connected (table + filters).

**Tech Stack:** Next.js, React, @tanstack/react-table, @tanstack/react-query, Tailwind CSS, shadcn/ui, Hugeicons

**Spec:** `docs/superpowers/specs/2026-03-25-brokerage-ui-design.md`

**IMPORTANT:** Do NOT commit unless explicitly asked.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `frontend/src/lib/types/accounts.ts` | Modify | Add `snaptradeUserId` field to `Account` |
| `frontend/src/lib/service/accounts.ts` | Modify | Add `snaptradeUserId` to GraphQL account fields |
| `frontend/src/hooks/brokerage.ts` | Modify | Add `useAutoSync` hook |
| `frontend/src/components/brokerage/brokerage-empty-state.tsx` | Create | CTA card for unlinked accounts |
| `frontend/src/components/brokerage/brokerage-filter-sidebar.tsx` | Create | Filter panel with sync status, type badges, date, search |
| `frontend/src/components/brokerage/brokerage-table.tsx` | Create | Transaction data table with pagination |
| `frontend/src/components/brokerage/brokerage-transactions.tsx` | Create | Connected state layout (sidebar + table) |
| `frontend/src/app/dashboard/brokerage/page.tsx` | Modify | Page shell with state routing |

---

### Task 1: Expose `snaptradeUserId` on the Account type

The backend already returns `snaptrade_user_id` on the Account GraphQL type, but the frontend `Account` interface and account query don't include it. This is needed to determine empty vs connected state.

**Files:**
- Modify: `frontend/src/lib/types/accounts.ts`
- Modify: `frontend/src/lib/service/accounts.ts`

- [ ] **Step 1: Add field to Account interface**

In `frontend/src/lib/types/accounts.ts`, add to the `Account` interface after `updatedAt`:

```typescript
snaptradeUserId: string | null;
```

- [ ] **Step 2: Add `snaptradeUserId` to all 4 GraphQL queries in `frontend/src/lib/service/accounts.ts`**

There is no shared fields constant — each query/mutation inlines its field list. Add `snaptradeUserId` after `updatedAt` in all four:

1. `ACCOUNTS_QUERY` (line 10-21) — add `snaptradeUserId` after `updatedAt` (line 19)
2. `ACCOUNT_QUERY` (line 24-37) — add `snaptradeUserId` after `updatedAt` (line 35)
3. `CREATE_ACCOUNT_MUTATION` (line 44-58) — add `snaptradeUserId` after `updatedAt` (line 55)
4. `UPDATE_ACCOUNT_MUTATION` (line 60-74) — add `snaptradeUserId` after `updatedAt` (line 71)

- [ ] **Step 3: Verify**

Run: `cd frontend && npx tsc --noEmit`
Expected: No type errors

---

### Task 2: Add `useAutoSync` hook

**Files:**
- Modify: `frontend/src/hooks/brokerage.ts`

- [ ] **Step 1: Add the useAutoSync hook**

Append to `frontend/src/hooks/brokerage.ts`:

```typescript
const SYNC_STALE_MS = 5 * 60 * 1000; // 5 minutes
const SYNC_STORAGE_KEY = "brokerage-last-sync";

type SyncState = "idle" | "syncing" | "synced" | "error";

export function useAutoSync(accountId: string | null) {
  const [syncState, setSyncState] = useState<SyncState>("idle");
  const [lastSyncTime, setLastSyncTime] = useState<string | null>(null);
  const didRun = useRef(false);
  const { mutateAsync } = useSyncBrokerageData();
  const mutateRef = useRef(mutateAsync);
  mutateRef.current = mutateAsync;

  const runSync = useCallback(async () => {
    if (!accountId) return;
    setSyncState("syncing");
    try {
      await mutateRef.current(accountId);
      const now = new Date().toISOString();
      sessionStorage.setItem(SYNC_STORAGE_KEY, now);
      setLastSyncTime(now);
      setSyncState("synced");
      toast.success("Brokerage data synced");
    } catch {
      setSyncState("error");
      toast.error("Failed to sync brokerage data");
    }
  }, [accountId]);

  useEffect(() => {
    if (!accountId || didRun.current) return;
    didRun.current = true;

    const stored = sessionStorage.getItem(SYNC_STORAGE_KEY);
    if (stored) {
      const elapsed = Date.now() - new Date(stored).getTime();
      if (elapsed < SYNC_STALE_MS) {
        setLastSyncTime(stored);
        setSyncState("synced");
        return;
      }
    }
    runSync();
  }, [accountId, runSync]);

  return { syncState, lastSyncTime, retrySync: runSync };
}
```

- [ ] **Step 2: Add missing imports**

Add `useState`, `useRef`, `useCallback`, `useEffect` to the React import at the top of the file. Also add `import { toast } from "sonner";`.

- [ ] **Step 3: Verify**

Run: `cd frontend && npx tsc --noEmit`
Expected: No type errors

---

### Task 3: Build `BrokerageEmptyState` component

**Files:**
- Create: `frontend/src/components/brokerage/brokerage-empty-state.tsx`

- [ ] **Step 1: Create the component**

```tsx
"use client";

import { HugeiconsIcon } from "@hugeicons/react";
import { BankIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

export function BrokerageEmptyState() {
  return (
    <div className="flex flex-1 items-center justify-center p-6">
      <Empty className="max-w-sm border-none">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <HugeiconsIcon icon={BankIcon} strokeWidth={2} />
          </EmptyMedia>
          <EmptyTitle>Connect your brokerage</EmptyTitle>
          <EmptyDescription>
            Link your brokerage account to automatically sync your transaction
            history, positions, and balances.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button size="sm">Connect Account</Button>
          <p className="text-xs text-muted-foreground">
            Supports 20+ brokerages via SnapTrade
          </p>
        </EmptyContent>
      </Empty>
    </div>
  );
}
```

Note: The "Connect Account" button is a placeholder — the SnapTrade portal redirect flow will be wired later. For now it renders the CTA visually.

- [ ] **Step 2: Verify**

Run: `cd frontend && npx tsc --noEmit`
Expected: No type errors. If `BankIcon` is not available in the free icon set, substitute with another icon like `Building06Icon` or similar.

---

### Task 4: Build `BrokerageTable` component

**Files:**
- Create: `frontend/src/components/brokerage/brokerage-table.tsx`

- [ ] **Step 1: Create the table component**

Follow the journal-table pattern. Key structure:

```tsx
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
  if (!iso) return "—";
  return new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" }).format(new Date(iso));
}

function fmtCurrency(value: number | null, currency = "USD"): string {
  if (value == null) return "—";
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
      if (!symbol) return <span className="text-muted-foreground">—</span>;
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
      return v !== 0 ? v.toLocaleString() : "—";
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
```

- [ ] **Step 2: Verify**

Run: `cd frontend && npx tsc --noEmit`
Expected: No type errors

---

### Task 5: Build `BrokerageFilterSidebar` component

**Files:**
- Create: `frontend/src/components/brokerage/brokerage-filter-sidebar.tsx`

- [ ] **Step 1: Create the sidebar component**

```tsx
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
          <p className="mt-0.5 text-[0.55rem] text-muted-foreground">Filtering current page</p>
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

      {/* Re-sync */}
      <div className="mt-auto border-t pt-3">
        <Button
          variant="outline"
          size="xs"
          className="w-full"
          onClick={onRetrySync}
          disabled={syncState === "syncing"}
        >
          Re-sync now
        </Button>
      </div>
    </aside>
  );
}
```

- [ ] **Step 2: Verify**

Run: `cd frontend && npx tsc --noEmit`
Expected: No type errors

---

### Task 6: Build `BrokerageTransactions` layout component

**Files:**
- Create: `frontend/src/components/brokerage/brokerage-transactions.tsx`

This component orchestrates filters, data fetching, client-side filtering, and renders the sidebar + table.

- [ ] **Step 1: Create the component**

```tsx
"use client";

import { useDeferredValue, useMemo, useState } from "react";
import { useActiveAccount } from "@/components/accounts";
import { BrokerageFilterSidebar } from "@/components/brokerage/brokerage-filter-sidebar";
import { BrokerageTable } from "@/components/brokerage/brokerage-table";
import { useAutoSync, useBrokerageTransactions } from "@/hooks/brokerage";
import type { TransactionFilters } from "@/lib/types/brokerage";

const DEFAULT_PAGE_SIZE = 20;

export function BrokerageTransactions() {
  const account = useActiveAccount();
  const accountId = account?.id ?? null;

  // Server-side filters (sent to GraphQL)
  const [filters, setFilters] = useState<TransactionFilters>({
    offset: 0,
    limit: DEFAULT_PAGE_SIZE,
  });

  // Client-side filters (applied to current page results)
  const [symbolSearch, setSymbolSearch] = useState("");
  const [descriptionSearch, setDescriptionSearch] = useState("");
  const deferredSymbol = useDeferredValue(symbolSearch);
  const deferredDesc = useDeferredValue(descriptionSearch);

  // Auto-sync on page load
  const { syncState, lastSyncTime, retrySync } = useAutoSync(accountId);

  // Fetch transactions
  const { data, isLoading, error } = useBrokerageTransactions(accountId, filters);
  const transactions = data?.data ?? [];
  const total = data?.total ?? 0;

  // Client-side filtering
  const filtered = useMemo(() => {
    let result = transactions;
    if (deferredSymbol) {
      const q = deferredSymbol.toLowerCase();
      result = result.filter(
        (t) =>
          t.symbol?.toLowerCase().includes(q) ||
          t.rawSymbol?.toLowerCase().includes(q),
      );
    }
    if (deferredDesc) {
      const q = deferredDesc.toLowerCase();
      result = result.filter((t) => t.description?.toLowerCase().includes(q));
    }
    return result;
  }, [transactions, deferredSymbol, deferredDesc]);

  const page = Math.floor((filters.offset ?? 0) / (filters.limit ?? DEFAULT_PAGE_SIZE));

  if (error) {
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <div className="rounded-xl border border-rose-200 bg-rose-50 p-6 text-center">
          <p className="font-medium text-rose-700">Failed to load transactions</p>
          <p className="mt-1 text-xs text-rose-600">{error.message}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-1 overflow-hidden">
      <BrokerageFilterSidebar
        filters={filters}
        onFiltersChange={setFilters}
        symbolSearch={symbolSearch}
        onSymbolSearchChange={setSymbolSearch}
        descriptionSearch={descriptionSearch}
        onDescriptionSearchChange={setDescriptionSearch}
        syncState={syncState}
        lastSyncTime={lastSyncTime}
        onRetrySync={retrySync}
      />
      <div className="flex flex-1 flex-col p-4 md:p-6">
        {/* Indeterminate progress bar during sync */}
        {syncState === "syncing" && (
          <div className="mb-3 h-0.5 overflow-hidden rounded-full bg-muted">
            <div className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
          </div>
        )}
        <BrokerageTable
          transactions={filtered}
          total={total}
          page={page}
          pageSize={filters.limit ?? DEFAULT_PAGE_SIZE}
          onPageChange={(p) => setFilters({ ...filters, offset: p * (filters.limit ?? DEFAULT_PAGE_SIZE) })}
          onPageSizeChange={(size) => setFilters({ ...filters, limit: size, offset: 0 })}
          isLoading={isLoading}
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify**

Run: `cd frontend && npx tsc --noEmit`
Expected: No type errors

---

### Task 7: Build the page shell

**Files:**
- Modify: `frontend/src/app/dashboard/brokerage/page.tsx`

- [ ] **Step 1: Write the page component**

```tsx
"use client";

import { AppSidebar } from "@/components/app-sidebar";
import { useActiveAccount } from "@/components/accounts";
import { BrokerageEmptyState } from "@/components/brokerage/brokerage-empty-state";
import { BrokerageTransactions } from "@/components/brokerage/brokerage-transactions";
import { SiteHeader } from "@/components/site-header";
import { GraphQLProvider } from "@/lib/client";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { ChatProvider } from "@/components/chat/chat-panel";

function BrokerageContent() {
  const account = useActiveAccount();
  const isLinked = !!account?.snaptradeUserId;

  if (!account) return null;

  return isLinked ? <BrokerageTransactions /> : <BrokerageEmptyState />;
}

export default function BrokeragePage() {
  return (
    <GraphQLProvider>
      <ChatProvider>
        <SidebarProvider
          style={
            {
              "--sidebar-width": "calc(var(--spacing) * 72)",
              "--header-height": "calc(var(--spacing) * 12)",
            } as React.CSSProperties
          }
        >
          <AppSidebar variant="inset" />
          <SidebarInset>
            <SiteHeader />
            <div className="flex flex-1 flex-col">
              <div className="@container/main flex flex-1 flex-col gap-2">
                <BrokerageContent />
              </div>
            </div>
          </SidebarInset>
        </SidebarProvider>
      </ChatProvider>
    </GraphQLProvider>
  );
}
```

- [ ] **Step 2: Verify the full build**

Run: `cd frontend && npx next build`
Expected: Build succeeds with no errors

---

### Task 8: Add navigation link

**Files:**
- Modify: `frontend/src/components/app-sidebar.tsx`
- Modify: `frontend/src/components/site-header.tsx`

- [ ] **Step 1: Add Brokerage to nav items in `frontend/src/components/app-sidebar.tsx`**

Add `BankIcon` to the import on line 17:
```typescript
import { DashboardSquare01Icon, File01Icon, CommandIcon, BookOpen01Icon, Notebook01Icon, BankIcon } from "@hugeicons/core-free-icons"
```

Then add this entry to the `data.navMain` array (line 19-49), after the Journal entry (line 41):
```typescript
    {
      title: "Brokerage",
      url: "/dashboard/brokerage",
      icon: (
        <HugeiconsIcon icon={BankIcon} strokeWidth={2} />
      ),
    },
```

- [ ] **Step 2: Add title mapping in `frontend/src/components/site-header.tsx`**

Add to the `ROUTE_TITLES` object (line 11-16):
```typescript
  "/dashboard/brokerage": "Brokerage",
```

- [ ] **Step 3: Verify**

Run: `cd frontend && npx next build`
Expected: Build succeeds. Navigate to `/dashboard/brokerage` in the browser to confirm the page renders.
