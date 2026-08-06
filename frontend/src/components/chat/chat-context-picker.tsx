"use client";

import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { TradstryMark } from "@/components/logo";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useChatStore } from "@/hooks/chat";
import { useJournalEntriesForWorkspace } from "@/hooks/journal";
import { usePlaybooks } from "@/hooks/playbook";
import { cn, formatPnl } from "@/lib/utils";

interface ChatContextPickerProps {
  workspaceId: string;
  onClose: () => void;
}

type ContextTab = "trades" | "playbooks" | "date-range";

function shortDate(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function toDateInputValue(date: Date) {
  const localDate = new Date(
    date.getTime() - date.getTimezoneOffset() * 60_000,
  );
  return localDate.toISOString().slice(0, 10);
}

function toggleId(ids: string[] | undefined, id: string) {
  const current = ids ?? [];
  return current.includes(id)
    ? current.filter((currentId) => currentId !== id)
    : [...current, id];
}

function SelectionIndicator({ selected }: { selected: boolean }) {
  return (
    <span
      className={cn(
        "flex size-[18px] shrink-0 items-center justify-center rounded-full border transition-colors",
        selected
          ? "border-foreground bg-foreground text-background"
          : "border-border bg-background group-hover:border-foreground/40",
      )}
      aria-hidden="true"
    >
      {selected ? (
        <svg
          aria-hidden="true"
          width="10"
          height="10"
          viewBox="0 0 12 12"
          fill="none"
        >
          <path
            d="m2.5 6 2.2 2.2 4.8-5"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      ) : null}
    </span>
  );
}

export function ChatContextPicker({
  workspaceId,
  onClose,
}: ChatContextPickerProps) {
  const { setPinnedContext, pinnedContext } = useChatStore();
  const { data: trades = [], isLoading: tradesLoading } =
    useJournalEntriesForWorkspace(workspaceId);
  const { data: playbooks = [], isLoading: playbooksLoading } = usePlaybooks();
  const [tab, setTab] = useState<ContextTab>("trades");
  const [query, setQuery] = useState("");
  const [from, setFrom] = useState(pinnedContext.dateRange?.from ?? "");
  const [to, setTo] = useState(pinnedContext.dateRange?.to ?? "");
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (tab !== "date-range") searchRef.current?.focus();
  }, [tab]);

  const filteredTrades = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return [...trades]
      .sort(
        (a, b) =>
          new Date(b.openDate).getTime() - new Date(a.openDate).getTime(),
      )
      .filter((trade) => {
        if (!normalizedQuery) return true;
        return `${trade.symbol} ${trade.symbolName} ${trade.tradeType}`
          .toLowerCase()
          .includes(normalizedQuery);
      });
  }, [query, trades]);

  const filteredPlaybooks = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return playbooks.filter((playbook) => {
      if (!normalizedQuery) return true;
      return `${playbook.name} ${playbook.edgeName}`
        .toLowerCase()
        .includes(normalizedQuery);
    });
  }, [playbooks, query]);

  function toggleTrade(tradeId: string) {
    setPinnedContext({
      ...pinnedContext,
      tradeIds: toggleId(pinnedContext.tradeIds, tradeId),
    });
  }

  function togglePlaybook(playbookId: string) {
    setPinnedContext({
      ...pinnedContext,
      playbookIds: toggleId(pinnedContext.playbookIds, playbookId),
    });
  }

  function applyDateRange() {
    if (!from || !to || from > to) return;
    setPinnedContext({ ...pinnedContext, dateRange: { from, to } });
  }

  function setDatePreset(days: number) {
    const end = new Date();
    const start = new Date();
    start.setDate(end.getDate() - (days - 1));
    setFrom(toDateInputValue(start));
    setTo(toDateInputValue(end));
  }

  function clearAll() {
    setPinnedContext({});
    setFrom("");
    setTo("");
  }

  function handleTabChange(value: string) {
    setTab(value as ContextTab);
    setQuery("");
  }

  const selectionCount =
    (pinnedContext.tradeIds?.length ?? 0) +
    (pinnedContext.playbookIds?.length ?? 0) +
    (pinnedContext.dateRange ? 1 : 0);

  return (
    <div
      role="dialog"
      aria-label="Add context"
      className="absolute right-3 bottom-full left-3 z-50 mb-2 overflow-hidden rounded-2xl border border-border/80 bg-popover shadow-xl shadow-black/10 animate-in fade-in-0 zoom-in-95 slide-in-from-bottom-2 duration-150"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          onClose();
        }
      }}
    >
      <div className="flex items-center gap-2.5 border-b border-border/70 px-3 py-2.5">
        <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-foreground text-background">
          <TradstryMark className="size-5" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-xs font-semibold">Add context</p>
          <p className="truncate text-[0.68rem] text-muted-foreground">
            {selectionCount > 0
              ? `${selectionCount} item${selectionCount === 1 ? "" : "s"} selected`
              : "Choose what Tradstry AI should focus on"}
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close context picker"
          className="flex size-7 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          <HugeiconsIcon icon={Cancel01Icon} className="size-3.5" />
        </button>
      </div>

      <Tabs value={tab} onValueChange={handleTabChange} className="gap-0">
        <TabsList className="mx-3 mt-2.5 grid h-8 w-auto grid-cols-3 rounded-lg bg-muted/70 p-0.5">
          <TabsTrigger
            value="trades"
            className="h-7 rounded-md px-2 text-[0.68rem] data-active:shadow-sm"
          >
            Trades
          </TabsTrigger>
          <TabsTrigger
            value="playbooks"
            className="h-7 rounded-md px-2 text-[0.68rem] data-active:shadow-sm"
          >
            Playbooks
          </TabsTrigger>
          <TabsTrigger
            value="date-range"
            className="h-7 rounded-md px-2 text-[0.68rem] data-active:shadow-sm"
          >
            Date range
          </TabsTrigger>
        </TabsList>

        {tab !== "date-range" ? (
          <div className="relative mx-3 mt-2.5">
            <svg
              aria-hidden="true"
              className="absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
              viewBox="0 0 20 20"
              fill="none"
            >
              <circle
                cx="8.5"
                cy="8.5"
                r="5.5"
                stroke="currentColor"
                strokeWidth="1.7"
              />
              <path
                d="m12.5 12.5 4 4"
                stroke="currentColor"
                strokeWidth="1.7"
                strokeLinecap="round"
              />
            </svg>
            <input
              ref={searchRef}
              type="text"
              inputMode="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={
                tab === "trades"
                  ? "Search symbol or company"
                  : "Search playbooks"
              }
              aria-label={
                tab === "trades" ? "Search trades" : "Search playbooks"
              }
              className="h-9 w-full rounded-lg border border-transparent bg-muted/55 pr-8 pl-8 text-xs outline-none placeholder:text-muted-foreground focus:border-ring/30 focus:bg-background focus:ring-2 focus:ring-ring/15"
            />
            {query ? (
              <button
                type="button"
                onClick={() => setQuery("")}
                aria-label="Clear search"
                className="absolute top-1/2 right-2 flex size-5 -translate-y-1/2 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
              >
                <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
              </button>
            ) : null}
          </div>
        ) : null}

        <TabsContent value="trades" className="p-0">
          <div className="flex items-center justify-between px-3 pt-2.5 pb-1.5 text-[0.65rem] text-muted-foreground">
            <span>Recent trades</span>
            <span className="tabular-nums">
              {filteredTrades.length} results
            </span>
          </div>
          <ScrollArea className="h-52">
            <div className="space-y-0.5 px-2 pb-2 pr-3">
              {tradesLoading ? (
                <p className="px-3 py-10 text-center text-xs text-muted-foreground">
                  Loading trades…
                </p>
              ) : filteredTrades.length === 0 ? (
                <div className="px-4 py-10 text-center">
                  <p className="text-xs font-medium">
                    {trades.length === 0 ? "No trades yet" : "No trades found"}
                  </p>
                  <p className="mt-1 text-[0.68rem] text-muted-foreground">
                    {trades.length === 0
                      ? "Trades from this workspace will appear here."
                      : "Try another symbol or company name."}
                  </p>
                </div>
              ) : (
                filteredTrades.map((trade) => {
                  const selected = Boolean(
                    pinnedContext.tradeIds?.includes(trade.id),
                  );
                  const profitable = trade.totalPl >= 0;
                  return (
                    <button
                      key={trade.id}
                      type="button"
                      aria-pressed={selected}
                      onClick={() => toggleTrade(trade.id)}
                      className={cn(
                        "group flex w-full items-center gap-2 rounded-xl border px-2.5 py-2 text-left transition-colors",
                        selected
                          ? "border-foreground/10 bg-muted"
                          : "border-transparent hover:bg-muted/60",
                      )}
                    >
                      <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background text-[0.68rem] font-bold tracking-wide ring-1 ring-border/70">
                        {trade.symbol.slice(0, 2)}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center gap-1.5">
                          <span className="truncate text-xs font-semibold tracking-wide">
                            {trade.symbol}
                          </span>
                          <span
                            className={cn(
                              "shrink-0 text-[0.58rem] font-medium uppercase",
                              trade.tradeType === "short"
                                ? "text-amber-600 dark:text-amber-400"
                                : "text-sky-600 dark:text-sky-400",
                            )}
                          >
                            {trade.tradeType}
                          </span>
                        </span>
                        <span className="block truncate text-[0.65rem] text-muted-foreground">
                          {shortDate(trade.openDate)}
                          {trade.symbolName ? ` · ${trade.symbolName}` : ""}
                        </span>
                      </span>
                      <span className="flex shrink-0 items-center gap-2">
                        <span
                          className={cn(
                            "text-[0.65rem] font-medium tabular-nums",
                            profitable
                              ? "text-emerald-600 dark:text-emerald-400"
                              : "text-rose-600 dark:text-rose-400",
                          )}
                        >
                          {formatPnl(trade.totalPl)}
                        </span>
                        <SelectionIndicator selected={selected} />
                      </span>
                    </button>
                  );
                })
              )}
            </div>
          </ScrollArea>
        </TabsContent>

        <TabsContent value="playbooks" className="p-0">
          <div className="flex items-center justify-between px-3 pt-2.5 pb-1.5 text-[0.65rem] text-muted-foreground">
            <span>Your playbooks</span>
            <span className="tabular-nums">
              {filteredPlaybooks.length} results
            </span>
          </div>
          <ScrollArea className="h-52">
            <div className="space-y-0.5 px-2 pb-2 pr-3">
              {playbooksLoading ? (
                <p className="px-3 py-10 text-center text-xs text-muted-foreground">
                  Loading playbooks…
                </p>
              ) : filteredPlaybooks.length === 0 ? (
                <div className="px-4 py-10 text-center">
                  <p className="text-xs font-medium">
                    {playbooks.length === 0
                      ? "No playbooks yet"
                      : "No playbooks found"}
                  </p>
                  <p className="mt-1 text-[0.68rem] text-muted-foreground">
                    {playbooks.length === 0
                      ? "Create a playbook to use it as AI context."
                      : "Try another name or edge."}
                  </p>
                </div>
              ) : (
                filteredPlaybooks.map((playbook) => {
                  const selected = Boolean(
                    pinnedContext.playbookIds?.includes(playbook.id),
                  );
                  return (
                    <button
                      key={playbook.id}
                      type="button"
                      aria-pressed={selected}
                      onClick={() => togglePlaybook(playbook.id)}
                      className={cn(
                        "group flex w-full items-center gap-2 rounded-xl border px-2.5 py-2 text-left transition-colors",
                        selected
                          ? "border-foreground/10 bg-muted"
                          : "border-transparent hover:bg-muted/60",
                      )}
                    >
                      <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background text-[0.68rem] font-bold ring-1 ring-border/70">
                        PB
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-xs font-semibold">
                          {playbook.name}
                        </span>
                        <span className="block truncate text-[0.65rem] text-muted-foreground">
                          {playbook.edgeName} · {playbook.tradeCount} trades
                        </span>
                      </span>
                      <span className="flex shrink-0 items-center gap-2">
                        <span className="text-[0.65rem] font-medium tabular-nums text-muted-foreground">
                          {Math.round(playbook.winRate)}%
                        </span>
                        <SelectionIndicator selected={selected} />
                      </span>
                    </button>
                  );
                })
              )}
            </div>
          </ScrollArea>
        </TabsContent>

        <TabsContent value="date-range" className="p-3">
          <div>
            <p className="text-[0.65rem] font-medium text-muted-foreground">
              Quick ranges
            </p>
            <div className="mt-1.5 grid grid-cols-3 gap-1.5">
              {[7, 30, 90].map((days) => (
                <button
                  key={days}
                  type="button"
                  onClick={() => setDatePreset(days)}
                  className="rounded-lg border border-border bg-background px-2 py-1.5 text-[0.68rem] font-medium transition-colors hover:bg-muted"
                >
                  Last {days} days
                </button>
              ))}
            </div>
          </div>

          <div className="mt-3 grid grid-cols-2 gap-2">
            <label className="flex flex-col gap-1 text-[0.65rem] font-medium text-muted-foreground">
              From
              <input
                type="date"
                value={from}
                onChange={(event) => setFrom(event.target.value)}
                className="h-9 min-w-0 rounded-lg border border-input bg-background px-2 text-[0.68rem] font-normal text-foreground outline-none focus:ring-2 focus:ring-ring/20"
              />
            </label>
            <label className="flex flex-col gap-1 text-[0.65rem] font-medium text-muted-foreground">
              To
              <input
                type="date"
                value={to}
                onChange={(event) => setTo(event.target.value)}
                className="h-9 min-w-0 rounded-lg border border-input bg-background px-2 text-[0.68rem] font-normal text-foreground outline-none focus:ring-2 focus:ring-ring/20"
              />
            </label>
          </div>
          {from && to && from > to ? (
            <p className="mt-2 text-[0.65rem] text-destructive">
              End date must be after the start date.
            </p>
          ) : null}
          <Button
            size="sm"
            className="mt-3 w-full"
            onClick={applyDateRange}
            disabled={!from || !to || from > to}
          >
            Use date range
          </Button>
          {pinnedContext.dateRange ? (
            <p className="mt-2 text-center text-[0.65rem] text-muted-foreground">
              Current: {pinnedContext.dateRange.from} –{" "}
              {pinnedContext.dateRange.to}
            </p>
          ) : null}
        </TabsContent>
      </Tabs>

      <div className="flex items-center justify-between border-t border-border/70 px-3 py-2">
        <button
          type="button"
          onClick={clearAll}
          disabled={selectionCount === 0}
          className="text-[0.68rem] font-medium text-muted-foreground transition-colors hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
        >
          Clear all
        </button>
        <Button size="sm" className="h-7 px-3 text-[0.68rem]" onClick={onClose}>
          Done{selectionCount > 0 ? ` · ${selectionCount}` : ""}
        </Button>
      </div>
    </div>
  );
}
