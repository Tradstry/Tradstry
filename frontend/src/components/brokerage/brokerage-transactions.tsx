"use client";

import { useDeferredValue, useMemo, useState } from "react";
import { useActiveAccount } from "@/components/accounts";
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

  // Auto-sync on page load
  const { syncState } = useAutoSync(accountId);

  // Fetch transactions
  const { data, isLoading, error } = useBrokerageTransactions(accountId, filters);
  const transactions = data?.data ?? [];
  const total = data?.total ?? 0;

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
    <div className="flex flex-1 flex-col overflow-hidden p-4 md:p-6">
      {syncState === "syncing" && (
        <div className="mb-3 h-0.5 overflow-hidden rounded-full bg-muted">
          <div className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
        </div>
      )}
      <BrokerageTable
        transactions={transactions}
        total={total}
        page={page}
        pageSize={filters.limit ?? DEFAULT_PAGE_SIZE}
        onPageChange={(p) => setFilters({ ...filters, offset: p * (filters.limit ?? DEFAULT_PAGE_SIZE) })}
        onPageSizeChange={(size) => setFilters({ ...filters, limit: size, offset: 0 })}
        isLoading={isLoading}
      />
    </div>
  );
}
