"use client";

import { useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { useActiveAccount } from "@/components/accounts";
import { BrokerageTable } from "@/components/brokerage/brokerage-table";
import { MergeTradesModal } from "@/components/brokerage/merge-trades-modal";
import { useBrokerageTransactions, useLinkedBrokerageTransactionIds } from "@/hooks/brokerage";
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

  // Selection state
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // Fetch transactions
  const { data, isLoading, error } = useBrokerageTransactions(accountId, filters);
  const transactions = data?.data ?? [];
  const total = data?.total ?? 0;

  // Fetch linked transaction IDs
  const { data: linkedIds } = useLinkedBrokerageTransactionIds(accountId);
  const linkedSet = useMemo(() => new Set(linkedIds ?? []), [linkedIds]);

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

  const selectedTxs = transactions.filter((t) => selectedIds.has(t.id));
  const symbols = new Set(selectedTxs.map((t) => t.symbol).filter(Boolean));
  const sameSymbol = symbols.size === 1;
  const symbol = sameSymbol ? [...symbols][0] : null;

  return (
    <div className="flex flex-1 flex-col overflow-hidden p-4 md:p-6">
      <BrokerageTable
        transactions={transactions}
        total={total}
        page={page}
        pageSize={filters.limit ?? DEFAULT_PAGE_SIZE}
        onPageChange={(p) => setFilters({ ...filters, offset: p * (filters.limit ?? DEFAULT_PAGE_SIZE) })}
        onPageSizeChange={(size) => setFilters({ ...filters, limit: size, offset: 0 })}
        isLoading={isLoading}
        linkedTransactionIds={linkedSet}
        selectedIds={selectedIds}
        onSelectedIdsChange={setSelectedIds}
      />
      {selectedIds.size >= 1 && (
        <DraggableBar>
          <span className="text-xs font-medium">
            {selectedIds.size} {symbol ?? "mixed"} selected
          </span>
          <MergeTradesModal
            selectedTransactions={selectedTxs}
            disabled={!sameSymbol}
            onSuccess={() => setSelectedIds(new Set())}
          />
        </DraggableBar>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Draggable floating bar
// ---------------------------------------------------------------------------

function DraggableBar({ children }: { children: React.ReactNode }) {
  const barRef = useRef<HTMLDivElement>(null);
  const dragState = useRef<{ startX: number; startY: number; origX: number; origY: number } | null>(null);

  function handlePointerDown(e: ReactPointerEvent<HTMLDivElement>) {
    if ((e.target as HTMLElement).closest("button, input, a, [role=dialog]")) return;
    if (!barRef.current) return;
    e.preventDefault();
    const rect = barRef.current.getBoundingClientRect();
    dragState.current = { startX: e.clientX, startY: e.clientY, origX: rect.left, origY: rect.top };
    // Switch from CSS centering to explicit positioning for drag
    barRef.current.style.left = `${rect.left}px`;
    barRef.current.style.top = `${rect.top}px`;
    barRef.current.style.right = "auto";
    barRef.current.style.bottom = "auto";
    barRef.current.style.margin = "0";
    barRef.current.style.transform = "none";
    barRef.current.style.cursor = "grabbing";
    document.addEventListener("pointermove", handlePointerMove);
    document.addEventListener("pointerup", handlePointerUp);
  }

  function handlePointerMove(e: globalThis.PointerEvent) {
    if (!dragState.current || !barRef.current) return;
    const dx = e.clientX - dragState.current.startX;
    const dy = e.clientY - dragState.current.startY;
    const x = Math.max(0, Math.min(window.innerWidth - barRef.current.offsetWidth, dragState.current.origX + dx));
    const y = Math.max(0, Math.min(window.innerHeight - barRef.current.offsetHeight, dragState.current.origY + dy));
    barRef.current.style.left = `${x}px`;
    barRef.current.style.top = `${y}px`;
  }

  function handlePointerUp() {
    dragState.current = null;
    if (barRef.current) barRef.current.style.cursor = "grab";
    document.removeEventListener("pointermove", handlePointerMove);
    document.removeEventListener("pointerup", handlePointerUp);
  }

  return (
    <div
      ref={barRef}
      onPointerDown={handlePointerDown}
      className="fixed inset-x-0 bottom-8 z-50 mx-auto flex w-fit items-center gap-2 rounded-lg border bg-background px-3 py-1.5 shadow-lg"
      style={{ cursor: "grab", touchAction: "none" }}
    >
      {children}
    </div>
  );
}
