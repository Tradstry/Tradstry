"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import * as React from "react";
import { useMemo, useState } from "react";
import { useActiveAccount } from "@/components/accounts";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCreateJournalEntry } from "@/hooks/journal";
import { usePlaybooks } from "@/hooks/playbook";
import { useGraphQL } from "@/lib/client";
import * as brokerageService from "@/lib/service/brokerage";
import type { BrokerageTransaction } from "@/lib/types/brokerage";
import type { TradeType } from "@/lib/types/journal";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------------------
// Auto-calculation helpers
// ---------------------------------------------------------------------------

function computeMergeDefaults(trades: BrokerageTransaction[]) {
  const sorted = [...trades].sort(
    (a, b) =>
      new Date(a.tradeDate ?? "").getTime() -
      new Date(b.tradeDate ?? "").getTime(),
  );

  const buys = sorted.filter((t) => t.transactionType === "BUY");
  const sells = sorted.filter((t) => t.transactionType === "SELL");

  const weightedAvg = (txs: BrokerageTransaction[]) => {
    let totalValue = 0;
    let totalUnits = 0;
    for (const t of txs) {
      const u = Math.abs(t.units);
      totalValue += t.price * u;
      totalUnits += u;
    }
    return totalUnits > 0 ? totalValue / totalUnits : 0;
  };

  const entryPrice = buys.length > 0 ? weightedAvg(buys) : weightedAvg(sells);
  const exitPrice = sells.length > 0 ? weightedAvg(sells) : 0;
  const positionSize =
    buys.reduce((sum, t) => sum + Math.abs(t.units), 0) ||
    sells.reduce((sum, t) => sum + Math.abs(t.units), 0);

  const firstType = sorted[0]?.transactionType;
  const tradeType: TradeType = firstType === "SELL" ? "short" : "long";

  // Convert to datetime-local format (YYYY-MM-DDTHH:mm)
  const openDate = toDatetimeLocal(sorted[0]?.tradeDate ?? "");
  const closeDate = toDatetimeLocal(sorted[sorted.length - 1]?.tradeDate ?? "");
  const symbol = sorted[0]?.symbol ?? "";
  const symbolName = sorted[0]?.symbolDescription ?? "";

  return {
    entryPrice,
    exitPrice,
    positionSize,
    tradeType,
    openDate,
    closeDate,
    symbol,
    symbolName,
  };
}

function toDatetimeLocal(iso: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function fmtDateShort(iso: string | null): string {
  if (!iso) return "—";
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
  }).format(new Date(iso));
}

// ---------------------------------------------------------------------------
// Field component
// ---------------------------------------------------------------------------

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Form state
// ---------------------------------------------------------------------------

type MergeFormState = {
  symbol: string;
  symbolName: string;
  openDate: string;
  closeDate: string;
  entryPrice: string;
  exitPrice: string;
  positionSize: string;
  stopLoss: string;
  tradeType: TradeType;
  mistakes: string;
  entryTactics: string;
  edgesSpotted: string;
  playbookId: string;
  notes: string;
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MergeTradesModal({
  selectedTransactions: passedSelected,
  prefillTransactionIds,
  trigger,
  disabled,
  onSuccess,
}: {
  /** Fully-loaded transactions selected upstream (the multi-select flow). */
  selectedTransactions?: BrokerageTransaction[];
  /** Alternative: hydrate transactions on-demand by id (the pending-trade flow). */
  prefillTransactionIds?: string[];
  /** Optional override for the default "Merge to Journal" trigger button. */
  trigger?: React.ReactNode;
  disabled?: boolean;
  onSuccess: () => void;
}) {
  const [open, setOpen] = useState(false);
  const account = useActiveAccount();
  const createTrade = useCreateJournalEntry();
  const playbooks = usePlaybooks();
  const queryClient = useQueryClient();
  const fetcher = useGraphQL();
  const [error, setError] = useState("");

  // When opened with prefillTransactionIds, fetch the transactions lazily.
  const prefillQuery = useQuery<BrokerageTransaction[]>({
    queryKey: ["brokerage-tx-by-ids", prefillTransactionIds],
    queryFn: () =>
      brokerageService.fetchBrokerageTransactionsByIds(
        fetcher,
        prefillTransactionIds ?? [],
      ),
    enabled:
      open && !!prefillTransactionIds && prefillTransactionIds.length > 0,
  });

  const selectedTransactions: BrokerageTransaction[] = useMemo(() => {
    if (prefillTransactionIds) {
      return prefillQuery.data ?? [];
    }
    return passedSelected ?? [];
  }, [prefillTransactionIds, prefillQuery.data, passedSelected]);

  const isPrefillLoading =
    !!prefillTransactionIds &&
    prefillQuery.isLoading &&
    selectedTransactions.length === 0;

  const defaults = useMemo(
    () => computeMergeDefaults(selectedTransactions),
    [selectedTransactions],
  );

  const [form, setForm] = useState<MergeFormState>({
    symbol: defaults.symbol,
    symbolName: defaults.symbolName,
    openDate: defaults.openDate,
    closeDate: defaults.closeDate,
    entryPrice: defaults.entryPrice.toFixed(4),
    exitPrice: defaults.exitPrice.toFixed(4),
    positionSize: defaults.positionSize.toFixed(2),
    stopLoss: "",
    tradeType: defaults.tradeType,
    mistakes: "",
    entryTactics: "",
    edgesSpotted: "",
    playbookId: "",
    notes: "",
  });

  // Seed the form exactly once per open, when transactions first resolve.
  // selectedTransactions is NOT referentially stable — the inline flow passes
  // an unmemoized filter() result and the prefill flow's query data is replaced
  // on background refetch — so keying the seed on it would blow away whatever
  // the user has typed every time the reference flips. The ref re-arms on close.
  const seededRef = React.useRef(false);
  React.useEffect(() => {
    if (!open) {
      seededRef.current = false;
      return;
    }
    if (seededRef.current) return;
    if (selectedTransactions.length === 0) return;
    const d = computeMergeDefaults(selectedTransactions);
    setForm({
      symbol: d.symbol,
      symbolName: d.symbolName,
      openDate: d.openDate,
      closeDate: d.closeDate,
      entryPrice: d.entryPrice.toFixed(4),
      exitPrice: d.exitPrice.toFixed(4),
      positionSize: d.positionSize.toFixed(2),
      stopLoss: "",
      tradeType: d.tradeType,
      mistakes: "",
      entryTactics: "",
      edgesSpotted: "",
      playbookId: "",
      notes: "",
    });
    setError("");
    seededRef.current = true;
  }, [open, selectedTransactions]);

  function setField<K extends keyof MergeFormState>(
    key: K,
    value: MergeFormState[K],
  ) {
    setForm((c) => ({ ...c, [key]: value }));
    if (error) setError("");
  }

  function validate(): string {
    if (!account) return "No active account";
    if (!form.symbol.trim()) return "Symbol is required";
    if (!form.openDate) return "Open date is required";
    if (!form.closeDate) return "Close date is required";
    if (!form.entryPrice.trim()) return "Entry price is required";
    if (!form.exitPrice.trim()) return "Exit price is required";
    if (!form.positionSize.trim()) return "Position size is required";
    if (!form.stopLoss.trim()) return "Stop loss is required";
    if (!form.mistakes.trim()) return "Mistakes is required";
    if (!form.entryTactics.trim()) return "Entry tactics is required";
    if (!form.edgesSpotted.trim()) return "Edges spotted is required";
    return "";
  }

  async function handleSubmit(e: React.SyntheticEvent<HTMLFormElement>) {
    e.preventDefault();
    const err = validate();
    if (err) {
      setError(err);
      return;
    }
    if (!account) return;

    try {
      await createTrade.mutateAsync({
        accountId: account.id,
        symbol: form.symbol.trim().toUpperCase(),
        symbolName: form.symbolName.trim() || undefined,
        openDate: form.openDate,
        closeDate: form.closeDate,
        entryPrice: Number(form.entryPrice),
        exitPrice: Number(form.exitPrice),
        positionSize: Number(form.positionSize),
        stopLoss: Number(form.stopLoss),
        tradeType: form.tradeType,
        mistakes: form.mistakes.trim(),
        entryTactics: form.entryTactics.trim(),
        edgesSpotted: form.edgesSpotted.trim(),
        playbookId: form.playbookId || undefined,
        notes: form.notes.trim() || undefined,
        brokerageTransactionIds: selectedTransactions.map((t) => t.id),
      });
      queryClient.invalidateQueries({ queryKey: ["linked-brokerage-tx-ids"] });
      setOpen(false);
      onSuccess();
    } catch (submitError) {
      setError(
        submitError instanceof Error
          ? submitError.message
          : "Failed to create journal entry",
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        {trigger ?? (
          <Button
            size="sm"
            disabled={disabled}
            title={disabled ? "Select trades of the same symbol" : undefined}
          >
            Merge to Journal
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-3xl max-h-[90vh] overflow-y-auto">
        {isPrefillLoading ? (
          <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
            Loading trade fills...
          </div>
        ) : (
          <form onSubmit={handleSubmit}>
            <DialogHeader>
              <DialogTitle>Merge Trades to Journal</DialogTitle>
              <DialogDescription>
                Merging {selectedTransactions.length} {defaults.symbol} trades
                into a journal entry.
              </DialogDescription>
            </DialogHeader>

            {/* Selected trades summary */}
            <div className="my-4 rounded-lg border bg-muted/30 p-3">
              <p className="mb-2 text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground">
                Selected Trades
              </p>
              <div className="flex flex-col gap-1">
                {[...selectedTransactions]
                  .sort(
                    (a, b) =>
                      new Date(a.tradeDate ?? "").getTime() -
                      new Date(b.tradeDate ?? "").getTime(),
                  )
                  .map((t) => (
                    <div key={t.id} className="flex items-center gap-3 text-xs">
                      <span className="w-16 text-muted-foreground">
                        {fmtDateShort(t.tradeDate)}
                      </span>
                      <span
                        className={`w-10 font-medium ${t.transactionType === "BUY" ? "text-emerald-600" : "text-rose-600"}`}
                      >
                        {t.transactionType}
                      </span>
                      <span className="w-16 tabular-nums">
                        {Math.abs(t.units)} units
                      </span>
                      <span className="tabular-nums text-muted-foreground">
                        @ ${t.price.toFixed(2)}
                      </span>
                    </div>
                  ))}
              </div>
            </div>

            {/* Journal entry form */}
            <div className="grid gap-4 py-2 md:grid-cols-2">
              <Field label="Symbol" htmlFor="merge-symbol">
                <Input
                  id="merge-symbol"
                  value={form.symbol}
                  onChange={(e) => setField("symbol", e.target.value)}
                />
              </Field>
              <Field label="Symbol Name" htmlFor="merge-symbol-name">
                <Input
                  id="merge-symbol-name"
                  value={form.symbolName}
                  onChange={(e) => setField("symbolName", e.target.value)}
                  placeholder="Optional, auto-fetched"
                />
              </Field>
              <Field label="Open Date" htmlFor="merge-open-date">
                <Input
                  id="merge-open-date"
                  type="datetime-local"
                  value={form.openDate}
                  onChange={(e) => setField("openDate", e.target.value)}
                />
              </Field>
              <Field label="Close Date" htmlFor="merge-close-date">
                <Input
                  id="merge-close-date"
                  type="datetime-local"
                  value={form.closeDate}
                  onChange={(e) => setField("closeDate", e.target.value)}
                />
              </Field>
              <Field label="Entry Price" htmlFor="merge-entry-price">
                <Input
                  id="merge-entry-price"
                  inputMode="decimal"
                  value={form.entryPrice}
                  onChange={(e) => setField("entryPrice", e.target.value)}
                  placeholder="0.00"
                />
              </Field>
              <Field label="Exit Price" htmlFor="merge-exit-price">
                <Input
                  id="merge-exit-price"
                  inputMode="decimal"
                  value={form.exitPrice}
                  onChange={(e) => setField("exitPrice", e.target.value)}
                  placeholder="0.00"
                />
              </Field>
              <Field label="Position Size" htmlFor="merge-position-size">
                <Input
                  id="merge-position-size"
                  inputMode="decimal"
                  value={form.positionSize}
                  onChange={(e) => setField("positionSize", e.target.value)}
                  placeholder="0.00"
                />
              </Field>
              <Field label="Stop Loss" htmlFor="merge-stop-loss">
                <Input
                  id="merge-stop-loss"
                  inputMode="decimal"
                  value={form.stopLoss}
                  onChange={(e) => setField("stopLoss", e.target.value)}
                  placeholder="Required"
                />
              </Field>
              <Field label="Trade Type">
                <Select
                  value={form.tradeType}
                  onValueChange={(v) => setField("tradeType", v as TradeType)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="long">Long</SelectItem>
                    <SelectItem value="short">Short</SelectItem>
                  </SelectContent>
                </Select>
              </Field>
              <Field label="Playbook (Optional)">
                <Select
                  value={form.playbookId || "__none__"}
                  onValueChange={(v) =>
                    setField("playbookId", v === "__none__" ? "" : v)
                  }
                  disabled={playbooks.isLoading}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="No playbook" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">No playbook</SelectItem>
                    {playbooks.data?.map((p) => (
                      <SelectItem key={p.id} value={p.id}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Field label="Mistakes" htmlFor="merge-mistakes">
                <Input
                  id="merge-mistakes"
                  value={form.mistakes}
                  onChange={(e) => setField("mistakes", e.target.value)}
                  placeholder="What went wrong?"
                />
              </Field>
              <Field label="Entry Tactics" htmlFor="merge-entry-tactics">
                <Input
                  id="merge-entry-tactics"
                  value={form.entryTactics}
                  onChange={(e) => setField("entryTactics", e.target.value)}
                  placeholder="Execution approach"
                />
              </Field>
              <Field label="Edges Spotted" htmlFor="merge-edges-spotted">
                <Input
                  id="merge-edges-spotted"
                  value={form.edgesSpotted}
                  onChange={(e) => setField("edgesSpotted", e.target.value)}
                  placeholder="Observed edge"
                />
              </Field>
              <Field label="Notes" htmlFor="merge-notes">
                <textarea
                  id="merge-notes"
                  value={form.notes}
                  onChange={(e) => setField("notes", e.target.value)}
                  placeholder="Optional notes"
                  rows={4}
                  className={cn(
                    "min-h-24 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30",
                  )}
                />
              </Field>
            </div>

            {error && <p className="pb-3 text-sm text-destructive">{error}</p>}

            <DialogFooter>
              <Button type="submit" disabled={createTrade.isPending}>
                {createTrade.isPending ? "Saving..." : "Create Journal Entry"}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
