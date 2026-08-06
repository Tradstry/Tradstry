"use client";

import * as React from "react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { Checkbox } from "@tradstry/app-ui/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@tradstry/app-ui/components/ui/dialog";
import { Input } from "@tradstry/app-ui/components/ui/input";
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { usePlaybooks } from "@tradstry/app-ui/hooks/playbook";
import type { JournalEntry } from "@tradstry/app-ui/lib/types/journal";
import { cn, formatPnl } from "@tradstry/app-ui/lib/utils";

const ANY = "__any__";

function shortDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** `YYYY-MM-DD` for a native date input, comparable as a string. */
function dayKey(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toISOString().slice(0, 10);
}

export function LinkTradesDialog({
  open,
  onOpenChange,
  trades,
  initialQuery = "",
  alreadyLinkedIds = [],
  onInsert,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  trades: JournalEntry[];
  /** Carried over from the `@` menu so "Browse all" keeps what you typed. */
  initialQuery?: string;
  alreadyLinkedIds?: string[];
  onInsert: (tradeIds: string[]) => void;
}) {
  const { data: playbooks } = usePlaybooks();

  const [query, setQuery] = React.useState(initialQuery);
  const [from, setFrom] = React.useState("");
  const [to, setTo] = React.useState("");
  const [side, setSide] = React.useState<string>(ANY);
  const [outcome, setOutcome] = React.useState<string>(ANY);
  const [tag, setTag] = React.useState<string>(ANY);
  const [playbook, setPlaybook] = React.useState<string>(ANY);
  const [selected, setSelected] = React.useState<Set<string>>(new Set());

  const linked = React.useMemo(
    () => new Set(alreadyLinkedIds),
    [alreadyLinkedIds],
  );

  // Reset every time the dialog opens so a previous session's filters and
  // selection never leak into a new insertion.
  React.useEffect(() => {
    if (!open) return;
    setQuery(initialQuery);
    setFrom("");
    setTo("");
    setSide(ANY);
    setOutcome(ANY);
    setTag(ANY);
    setPlaybook(ANY);
    setSelected(new Set());
  }, [open, initialQuery]);

  const tagNames = React.useMemo(() => {
    const names = new Set<string>();
    for (const trade of trades) {
      for (const t of trade.tags ?? []) names.add(t.name);
    }
    return [...names].sort();
  }, [trades]);

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase();
    return trades
      .filter((t) => {
        if (q) {
          const haystack = `${t.symbol} ${t.symbolName ?? ""}`.toLowerCase();
          if (!haystack.includes(q)) return false;
        }
        const day = dayKey(t.openDate);
        if (from && day < from) return false;
        if (to && day > to) return false;
        if (side !== ANY && t.tradeType !== side) return false;
        if (outcome !== ANY && t.status !== outcome) return false;
        if (tag !== ANY && !(t.tags ?? []).some((x) => x.name === tag)) {
          return false;
        }
        if (playbook !== ANY && t.playbookId !== playbook) return false;
        return true;
      })
      .sort(
        (a, b) =>
          new Date(b.openDate).getTime() - new Date(a.openDate).getTime(),
      );
  }, [trades, query, from, to, side, outcome, tag, playbook]);

  const selectable = filtered.filter((t) => !linked.has(t.id));
  const allSelected =
    selectable.length > 0 && selectable.every((t) => selected.has(t.id));

  const net = React.useMemo(() => {
    let sum = 0;
    for (const t of trades) if (selected.has(t.id)) sum += t.totalPl;
    return sum;
  }, [trades, selected]);

  function toggle(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  /** Select-all applies to the current filter, which is what makes a weekly
   *  review two clicks rather than eight. */
  function toggleAll() {
    setSelected((prev) => {
      const next = new Set(prev);
      if (allSelected) {
        for (const t of selectable) next.delete(t.id);
      } else {
        for (const t of selectable) next.add(t.id);
      }
      return next;
    });
  }

  function insert() {
    if (selected.size === 0) return;
    const ordered = filtered.filter((t) => selected.has(t.id)).map((t) => t.id);
    onInsert(ordered);
    onOpenChange(false);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[min(42rem,calc(100svh-3rem))] flex-col overflow-hidden sm:max-w-3xl">
        <DialogHeader className="shrink-0">
          <DialogTitle>Link trades</DialogTitle>
          <DialogDescription>
            Pick the trades to insert as a table. Filters narrow the list;
            selecting all applies to what you have filtered to.
          </DialogDescription>
        </DialogHeader>

        <div className="grid shrink-0 gap-2">
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search symbol or company…"
            className="h-8"
          />
          <div className="flex flex-wrap items-center gap-2">
            <Input
              type="date"
              value={from}
              onChange={(e) => setFrom(e.target.value)}
              className="h-8 w-[9.5rem]"
              aria-label="From date"
            />
            <span className="text-xs text-muted-foreground">→</span>
            <Input
              type="date"
              value={to}
              onChange={(e) => setTo(e.target.value)}
              className="h-8 w-[9.5rem]"
              aria-label="To date"
            />

            <Select value={side} onValueChange={setSide}>
              <SelectTrigger className="h-8 w-[7rem]">
                <SelectValue placeholder="Side" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ANY}>Any side</SelectItem>
                <SelectItem value="long">Long</SelectItem>
                <SelectItem value="short">Short</SelectItem>
              </SelectContent>
            </Select>

            <Select value={outcome} onValueChange={setOutcome}>
              <SelectTrigger className="h-8 w-[7.5rem]">
                <SelectValue placeholder="Outcome" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ANY}>Any outcome</SelectItem>
                <SelectItem value="profit">Wins</SelectItem>
                <SelectItem value="loss">Losses</SelectItem>
              </SelectContent>
            </Select>

            <Select value={tag} onValueChange={setTag}>
              <SelectTrigger className="h-8 w-[8rem]">
                <SelectValue placeholder="Tag" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ANY}>Any tag</SelectItem>
                {tagNames.map((name) => (
                  <SelectItem key={name} value={name}>
                    {name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <Select value={playbook} onValueChange={setPlaybook}>
              <SelectTrigger className="h-8 w-[9rem]">
                <SelectValue placeholder="Playbook" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ANY}>Any playbook</SelectItem>
                {(playbooks ?? []).map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    {p.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-3 border-y border-border py-1.5 pl-3 pr-1 text-xs text-muted-foreground">
          <Checkbox
            checked={allSelected}
            onCheckedChange={toggleAll}
            disabled={selectable.length === 0}
            aria-label="Select all filtered trades"
          />
          <span>
            {filtered.length} {filtered.length === 1 ? "trade" : "trades"}
          </span>
        </div>

        <ScrollArea className="min-h-0 flex-1" type="always">
          <div className="divide-y divide-border/50">
            {filtered.length === 0 ? (
              <p className="px-3 py-10 text-center text-xs text-muted-foreground">
                No trades match these filters.
              </p>
            ) : null}
            {filtered.map((trade) => {
              const isLinked = linked.has(trade.id);
              const isProfit = trade.status === "profit";
              return (
                <button
                  key={trade.id}
                  type="button"
                  disabled={isLinked}
                  onClick={() => toggle(trade.id)}
                  aria-pressed={isLinked || selected.has(trade.id)}
                  className={cn(
                    "flex w-full items-center gap-3 px-3 py-2 text-left text-xs",
                    isLinked
                      ? "cursor-not-allowed opacity-50"
                      : "cursor-pointer hover:bg-accent/50",
                  )}
                >
                  <Checkbox
                    checked={isLinked || selected.has(trade.id)}
                    disabled={isLinked}
                    tabIndex={-1}
                    className="pointer-events-none"
                  />
                  <span className="w-16 shrink-0 font-semibold tracking-wide">
                    {trade.symbol}
                  </span>
                  <span
                    className={cn(
                      "w-12 shrink-0 text-[0.65rem] uppercase tracking-wide",
                      trade.tradeType === "short"
                        ? "text-amber-600 dark:text-amber-400"
                        : "text-sky-600 dark:text-sky-400",
                    )}
                  >
                    {trade.tradeType}
                  </span>
                  <span className="w-14 shrink-0 text-muted-foreground">
                    {shortDate(trade.openDate)}
                  </span>
                  <span className="flex-1 truncate tabular-nums text-muted-foreground">
                    {trade.entryPrice} → {trade.exitPrice}
                  </span>
                  <span
                    className={cn(
                      "shrink-0 font-medium tabular-nums",
                      isProfit
                        ? "text-emerald-600 dark:text-emerald-400"
                        : "text-rose-600 dark:text-rose-400",
                    )}
                  >
                    {formatPnl(trade.totalPl, { precision: "cents" })}
                  </span>
                </button>
              );
            })}
          </div>
        </ScrollArea>

        <div className="flex shrink-0 items-center justify-between gap-3 border-t border-border pt-3">
          <p className="text-xs text-muted-foreground tabular-nums">
            {selected.size} selected
            {selected.size > 0 ? (
              <>
                {" · net "}
                <span
                  className={cn(
                    "font-medium",
                    net >= 0
                      ? "text-emerald-600 dark:text-emerald-400"
                      : "text-rose-600 dark:text-rose-400",
                  )}
                >
                  {formatPnl(net, { precision: "cents" })}
                </span>
              </>
            ) : null}
          </p>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button size="sm" onClick={insert} disabled={selected.size === 0}>
              Insert
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
