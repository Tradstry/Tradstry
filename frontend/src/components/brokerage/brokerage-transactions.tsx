"use client";

import {
  type PointerEvent as ReactPointerEvent,
  useMemo,
  useRef,
  useState,
} from "react";
import { useActiveAccount } from "@/components/accounts";
import { BrokerageTable } from "@/components/brokerage/brokerage-table";
import { MergeTradesModal } from "@/components/brokerage/merge-trades-modal";
import { PendingTrades } from "@/components/brokerage/pending-trades";
import {
  useBrokerageTransactions,
  useLinkedBrokerageTransactionIds,
} from "@/hooks/brokerage";
import type { TransactionFilters } from "@/lib/types/brokerage";
import { cn } from "@/lib/utils";

const DEFAULT_PAGE_SIZE = 100;

type BrokerageTab = "pending" | "all";

type DateRange = "1W" | "1M" | "3M" | "6M" | "YTD" | "1Y" | "Max";

function getStartDate(range: DateRange): string | undefined {
  if (range === "Max") return undefined;
  const now = new Date();
  if (range === "YTD") {
    return `${now.getFullYear()}-01-01`;
  }
  const offsets: Record<string, number> = {
    "1W": 7,
    "1M": 30,
    "3M": 90,
    "6M": 180,
    "1Y": 365,
  };
  const d = new Date(now);
  d.setDate(d.getDate() - offsets[range]);
  return d.toISOString().slice(0, 10);
}

export function BrokerageTransactions() {
  const account = useActiveAccount();
  const accountId = account?.id ?? null;

  const [tab, setTab] = useState<BrokerageTab>("pending");
  const [dateRange, setDateRange] = useState<DateRange>("Max");

  // Server-side filters (sent to GraphQL)
  const [filters, setFilters] = useState<TransactionFilters>({
    offset: 0,
    limit: DEFAULT_PAGE_SIZE,
    sortBy: "symbol",
  });

  // Track page offsets so "previous" works after trimming
  const [pageOffsets, setPageOffsets] = useState<number[]>([0]);

  function handleDateRangeChange(range: DateRange) {
    setDateRange(range);
    setPageOffsets([0]);
    setFilters((prev) => ({
      ...prev,
      startDate: getStartDate(range),
      offset: 0,
    }));
  }

  // Selection state
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // Fetch transactions
  const { data, isLoading, error } = useBrokerageTransactions(
    accountId,
    filters,
  );
  const rawTransactions = data?.data ?? [];
  const total = data?.total ?? 0;

  // Trim trailing month+symbol group if it might be split across pages.
  // The backend sorts by month DESC, symbol ASC, so a split only happens at the end.
  const { displayTransactions, nextOffset } = useMemo(() => {
    const offset = filters.offset ?? 0;
    if (!rawTransactions.length) {
      return { displayTransactions: rawTransactions, nextOffset: offset };
    }
    const isLastPage = offset + rawTransactions.length >= total;
    if (isLastPage) {
      return {
        displayTransactions: rawTransactions,
        nextOffset: offset + rawTransactions.length,
      };
    }
    const groupKey = (tx: (typeof rawTransactions)[0]) =>
      `${tx.tradeDate?.slice(0, 7) ?? ""}:${tx.symbol ?? ""}`;
    const lastKey = groupKey(rawTransactions[rawTransactions.length - 1]);
    let trimIndex = rawTransactions.length;
    for (let i = rawTransactions.length - 1; i >= 0; i--) {
      if (groupKey(rawTransactions[i]) !== lastKey) {
        trimIndex = i + 1;
        break;
      }
      if (i === 0) {
        // Entire page is one month+symbol — don't trim
        return {
          displayTransactions: rawTransactions,
          nextOffset: offset + rawTransactions.length,
        };
      }
    }
    return {
      displayTransactions: rawTransactions.slice(0, trimIndex),
      nextOffset: offset + trimIndex,
    };
  }, [rawTransactions, filters.offset, total]);

  const transactions = displayTransactions;

  // Fetch linked transaction IDs
  const { data: linkedIds } = useLinkedBrokerageTransactionIds(accountId);
  const linkedSet = useMemo(() => new Set(linkedIds ?? []), [linkedIds]);

  const currentPage = pageOffsets.length - 1;
  const hasNextPage = nextOffset < total;
  const hasPrevPage = currentPage > 0;

  if (error) {
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <div className="rounded-xl border border-rose-200 bg-rose-50 p-6 text-center">
          <p className="font-medium text-rose-700">
            Failed to load transactions
          </p>
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
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="border-b bg-background px-4 pt-3 md:px-6">
        <div className="flex gap-1">
          <TabButton
            active={tab === "pending"}
            onClick={() => setTab("pending")}
          >
            Pending
          </TabButton>
          <TabButton active={tab === "all"} onClick={() => setTab("all")}>
            All transactions
          </TabButton>
        </div>
      </div>

      {tab === "pending" ? (
        <PendingTrades />
      ) : (
        <div className="flex flex-1 flex-col overflow-hidden p-4 md:p-6">
          <BrokerageTable
            transactions={transactions}
            total={total}
            page={currentPage}
            pageSize={filters.limit ?? DEFAULT_PAGE_SIZE}
            hasNextPage={hasNextPage}
            hasPrevPage={hasPrevPage}
            onNextPage={() => {
              setPageOffsets((prev) => [...prev, nextOffset]);
              setFilters((prev) => ({ ...prev, offset: nextOffset }));
            }}
            onPrevPage={() => {
              setPageOffsets((prev) => {
                const next = prev.slice(0, -1);
                setFilters((f) => ({ ...f, offset: next[next.length - 1] }));
                return next;
              });
            }}
            onPageSizeChange={(size) => {
              setPageOffsets([0]);
              setFilters({ ...filters, limit: size, offset: 0 });
            }}
            isLoading={isLoading}
            linkedTransactionIds={linkedSet}
            selectedIds={selectedIds}
            onSelectedIdsChange={setSelectedIds}
            dateRange={dateRange}
            onDateRangeChange={handleDateRangeChange}
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
      )}
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "relative px-3 py-2 text-xs font-medium transition-colors",
        active
          ? "text-foreground after:absolute after:inset-x-0 after:-bottom-px after:h-0.5 after:bg-foreground"
          : "text-muted-foreground hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Draggable floating bar
// ---------------------------------------------------------------------------

function DraggableBar({ children }: { children: React.ReactNode }) {
  const barRef = useRef<HTMLDivElement>(null);
  const dragState = useRef<{
    startX: number;
    startY: number;
    origX: number;
    origY: number;
  } | null>(null);

  function handlePointerDown(e: ReactPointerEvent<HTMLDivElement>) {
    if ((e.target as HTMLElement).closest("button, input, a, [role=dialog]"))
      return;
    if (!barRef.current) return;
    e.preventDefault();
    const rect = barRef.current.getBoundingClientRect();
    dragState.current = {
      startX: e.clientX,
      startY: e.clientY,
      origX: rect.left,
      origY: rect.top,
    };
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
    const x = Math.max(
      0,
      Math.min(
        window.innerWidth - barRef.current.offsetWidth,
        dragState.current.origX + dx,
      ),
    );
    const y = Math.max(
      0,
      Math.min(
        window.innerHeight - barRef.current.offsetHeight,
        dragState.current.origY + dy,
      ),
    );
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
