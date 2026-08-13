"use client";

import { BrokerageTable } from "@tradstry/app-ui/components/brokerage/brokerage-table";
import { MergeTradesModal } from "@tradstry/app-ui/components/brokerage/merge-trades-modal";
import { PendingTrades } from "@tradstry/app-ui/components/brokerage/pending-trades";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import {
  useBrokerageTransactions,
  useBrokerageTransactionsByIds,
  useLinkedBrokerageTransactionIds,
} from "@tradstry/app-ui/hooks/brokerage";
import type { AnalyticsRange } from "@tradstry/app-ui/lib/types/analytics";
import type { TransactionFilters } from "@tradstry/app-ui/lib/types/brokerage";
import { cn } from "@tradstry/app-ui/lib/utils";
import {
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

const DEFAULT_PAGE_SIZE = 100;

const JOURNALLED_FILTER_STORAGE_KEY = "brokerage-journalled-filter";

type BrokerageTab = "pending" | "all" | "journalled";

type JournalledFilter = "journalled" | "unjournalled";

export function BrokerageTransactions() {
  const account = useActiveWorkspace();
  const workspaceId = account?.id ?? null;

  const [tab, setTab] = useState<BrokerageTab>("pending");
  const [dateRange, setDateRange] = useState<AnalyticsRange>("ALL");
  // Sub-filter for the "Journalled" tab: linked vs not-yet-linked trades.
  // Persisted to localStorage so the last choice is remembered across visits.
  const [journalledFilter, setJournalledFilter] = useState<JournalledFilter>(
    () => {
      if (typeof window === "undefined") return "journalled";
      return window.localStorage.getItem(JOURNALLED_FILTER_STORAGE_KEY) ===
        "unjournalled"
        ? "unjournalled"
        : "journalled";
    },
  );

  // Server-side filters (sent to GraphQL)
  const [filters, setFilters] = useState<TransactionFilters>({
    offset: 0,
    limit: DEFAULT_PAGE_SIZE,
    sortBy: "symbol",
  });

  // Track page offsets so "previous" works after trimming
  const [pageOffsets, setPageOffsets] = useState<number[]>([0]);

  function handleDateRangeChange(range: AnalyticsRange) {
    setDateRange(range);
    setPageOffsets([0]);
    setFilters((prev) => ({
      ...prev,
      range,
      offset: 0,
    }));
  }

  // Selection state
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // Symbol search drives the server-side filter (filters.symbol).
  const [symbolSearch, setSymbolSearch] = useState("");

  function handleTabChange(next: BrokerageTab) {
    if (next === tab) return;
    setTab(next);
    setSelectedIds(new Set());
    setPageOffsets([0]);
    const isJournalled =
      next === "journalled" ? journalledFilter === "journalled" : undefined;
    setFilters((prev) => ({ ...prev, isJournalled, offset: 0 }));
  }

  function handleJournalledFilterChange(next: JournalledFilter) {
    if (next === journalledFilter) return;
    setJournalledFilter(next);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(JOURNALLED_FILTER_STORAGE_KEY, next);
    }
    setSelectedIds(new Set());
    setPageOffsets([0]);
    setFilters((prev) => ({
      ...prev,
      isJournalled: next === "journalled",
      offset: 0,
    }));
  }

  // Fetch transactions
  const { data, isLoading, error } = useBrokerageTransactions(
    workspaceId,
    filters,
  );

  // Debounce symbol search into the server-side filter (resets pagination).
  useEffect(() => {
    const handle = setTimeout(() => {
      const next = symbolSearch.trim().toUpperCase() || undefined;
      setPageOffsets([0]);
      setFilters((prev) =>
        prev.symbol === next ? prev : { ...prev, symbol: next, offset: 0 },
      );
    }, 300);
    return () => clearTimeout(handle);
  }, [symbolSearch]);

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
  const { data: linkedIds } = useLinkedBrokerageTransactionIds(workspaceId);
  const linkedSet = useMemo(() => new Set(linkedIds ?? []), [linkedIds]);

  const currentPage = pageOffsets.length - 1;
  const hasNextPage = nextOffset < total;
  const hasPrevPage = currentPage > 0;

  // A selection can span several server-side pages. The current page's
  // `transactions` only covers on-screen rows, so hydrate the full selected set
  // by id and union it with the page-local rows. The union keeps same-page
  // selections instant (no fetch wait) while still pulling in off-page picks —
  // without it, the merge silently drops any selected trade not on this page.
  const selectedIdList = useMemo(() => [...selectedIds].sort(), [selectedIds]);
  const { data: hydratedSelected } =
    useBrokerageTransactionsByIds(selectedIdList);

  const selectedTxs = useMemo(() => {
    const byId = new Map<string, (typeof transactions)[number]>();
    for (const t of transactions) {
      if (selectedIds.has(t.id)) byId.set(t.id, t);
    }
    for (const t of hydratedSelected ?? []) {
      if (selectedIds.has(t.id)) byId.set(t.id, t);
    }
    return [...byId.values()];
  }, [transactions, hydratedSelected, selectedIds]);
  const symbols = new Set(selectedTxs.map((t) => t.symbol).filter(Boolean));
  const sameSymbol = symbols.size === 1;
  const symbol = sameSymbol ? [...symbols][0] : null;

  if (error) {
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <div className="rounded-xl border border-rose-200 dark:border-rose-900 bg-rose-50 dark:bg-rose-950/50 p-6 text-center">
          <p className="font-medium text-rose-700 dark:text-rose-300">
            Failed to load transactions
          </p>
          <p className="mt-1 text-xs text-rose-600 dark:text-rose-400">
            {error.message}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden bg-muted/10">
      <div className="shrink-0 border-b bg-background px-4 md:px-6">
        <div
          aria-label="Brokerage views"
          role="tablist"
          className="flex h-12 items-end gap-6 overflow-x-auto"
        >
          <TabButton
            active={tab === "pending"}
            onClick={() => handleTabChange("pending")}
          >
            Pending
          </TabButton>
          <TabButton
            active={tab === "all"}
            onClick={() => handleTabChange("all")}
          >
            All transactions
          </TabButton>
          <TabButton
            active={tab === "journalled"}
            onClick={() => handleTabChange("journalled")}
          >
            Journalled
          </TabButton>
        </div>
      </div>

      {tab === "pending" ? (
        <PendingTrades />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden p-3 md:p-5 xl:p-6">
          <BrokerageTable
            transactions={transactions}
            symbolSearch={symbolSearch}
            onSymbolSearchChange={setSymbolSearch}
            total={total}
            offset={filters.offset ?? 0}
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
            scopeControl={
              tab === "journalled" ? (
                <fieldset className="flex items-center rounded-lg border bg-background p-0.5">
                  <legend className="sr-only">Journal status</legend>
                  {(
                    [
                      ["journalled", "In journal"],
                      ["unjournalled", "Needs journal"],
                    ] as const
                  ).map(([value, label]) => (
                    <button
                      key={value}
                      type="button"
                      aria-pressed={journalledFilter === value}
                      onClick={() => handleJournalledFilterChange(value)}
                      className={cn(
                        "h-7 rounded-md px-2.5 text-[0.6875rem] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/30",
                        journalledFilter === value
                          ? "bg-foreground text-background shadow-sm"
                          : "text-muted-foreground hover:bg-muted hover:text-foreground",
                      )}
                    >
                      {label}
                    </button>
                  ))}
                </fieldset>
              ) : undefined
            }
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
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={cn(
        "relative h-12 shrink-0 px-0.5 pb-3 pt-4 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/30",
        active
          ? "text-foreground after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:rounded-full after:bg-sky-500"
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
      className="fixed inset-x-0 bottom-8 z-50 mx-auto flex w-fit items-center gap-3 rounded-xl border border-border/80 bg-background px-3 py-2 shadow-[0_16px_40px_rgb(0_0_0/0.16)]"
      style={{ cursor: "grab", touchAction: "none" }}
    >
      {children}
    </div>
  );
}
