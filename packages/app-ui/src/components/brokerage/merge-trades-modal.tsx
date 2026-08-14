"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { PrinciplePicker } from "@tradstry/app-ui/components/journal/principle-picker";
import { TagPicker } from "@tradstry/app-ui/components/journal/tag-picker";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { DateTimePicker } from "@tradstry/app-ui/components/ui/date-time-picker";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@tradstry/app-ui/components/ui/dialog";
import { Input } from "@tradstry/app-ui/components/ui/input";
import { Label } from "@tradstry/app-ui/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import {
  useCreateJournalEntry,
  usePublishBrokerageEpisodeReview,
} from "@tradstry/app-ui/hooks/journal";
import { usePlaybooks } from "@tradstry/app-ui/hooks/playbook";
import {
  usePositionCalculatorPlans,
  useTradeReviewInbox,
} from "@tradstry/app-ui/hooks/position-calculator";
import { usePrinciples } from "@tradstry/app-ui/hooks/principle";
import { useTagCategories } from "@tradstry/app-ui/hooks/tags";
import { capture, EVENTS } from "@tradstry/app-ui/lib/analytics/events";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as brokerageService from "@tradstry/app-ui/lib/service/brokerage";
import type { BrokerageTransaction } from "@tradstry/app-ui/lib/types/brokerage";
import type { TradeType } from "@tradstry/app-ui/lib/types/journal";
import type { TradeReviewMatchSuggestion } from "@tradstry/app-ui/lib/types/position-calculator";
import { cn } from "@tradstry/app-ui/lib/utils";
import * as React from "react";
import { useMemo, useState } from "react";

// ---------------------------------------------------------------------------
// Auto-calculation helpers
// ---------------------------------------------------------------------------

function computeMergeDefaults(trades: BrokerageTransaction[]) {
  const sorted = [...trades].sort(
    (a, b) =>
      new Date(a.tradeDate ?? "").getTime() -
      new Date(b.tradeDate ?? "").getTime(),
  );

  // Prefix match, not equality: option fills arrive as BUY_TO_OPEN / SELL_TO_CLOSE.
  const isBuy = (t: BrokerageTransaction) =>
    t.transactionType.toUpperCase().startsWith("BUY");
  const isSell = (t: BrokerageTransaction) =>
    t.transactionType.toUpperCase().startsWith("SELL");
  const buys = sorted.filter(isBuy);
  const sells = sorted.filter(isSell);

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

  const tradeType: TradeType =
    sorted[0] && isSell(sorted[0]) ? "short" : "long";

  // Convert to datetime-local format (YYYY-MM-DDTHH:mm)
  const openDate = toDatetimeLocal(sorted[0]?.tradeDate ?? "");
  const closeDate = toDatetimeLocal(sorted[sorted.length - 1]?.tradeDate ?? "");

  // One contract = 100 shares (10 for minis). Prices stay per-share and
  // positionSize stays in contracts; the multiplier drives dollar P&L.
  const contractMultiplier = Math.max(
    1,
    ...trades.map((t) => t.contractMultiplier ?? 1),
  );
  const isOption = contractMultiplier !== 1;
  const first = sorted[0];
  // Group options under the underlying ticker; keep the readable contract in the name.
  const symbol =
    (isOption ? first?.underlyingSymbol : first?.symbol) ?? first?.symbol ?? "";
  const symbolName = first?.symbolDescription ?? "";

  return {
    entryPrice,
    exitPrice,
    positionSize,
    tradeType,
    openDate,
    closeDate,
    symbol,
    symbolName,
    contractMultiplier,
    isOption,
  };
}

function toDatetimeLocal(iso: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
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
  className,
}: {
  label: string;
  htmlFor?: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("grid gap-2", className)}>
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
  // "set" => stopLoss holds a price; "none" => the trade had no stop loss.
  stopLossMode: "set" | "none";
  tradeType: TradeType;
  playbookId: string;
  notes: string;
  violatedPrincipleIds: string[];
  planId: string;
  planAdherence: string;
  lesson: string;
};

function parseSuggestions(value: string | undefined) {
  if (!value) return [];
  try {
    return JSON.parse(value) as TradeReviewMatchSuggestion[];
  } catch {
    return [];
  }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MergeTradesModal({
  selectedTransactions: passedSelected,
  prefillTransactionIds,
  trigger,
  disabled,
  onSuccess,
  episodeId,
}: {
  /** Fully-loaded transactions selected upstream (the multi-select flow). */
  selectedTransactions?: BrokerageTransaction[];
  /** Alternative: hydrate transactions on-demand by id (the pending-trade flow). */
  prefillTransactionIds?: string[];
  /** Optional override for the default "Merge to Journal" trigger button. */
  trigger?: React.ReactNode;
  disabled?: boolean;
  onSuccess: () => void;
  /** Deterministic broker episode. When present, execution facts are locked. */
  episodeId?: string;
}) {
  const [open, setOpen] = useState(false);
  const account = useActiveWorkspace();
  const createTrade = useCreateJournalEntry();
  const publishEpisode = usePublishBrokerageEpisodeReview();
  const reviewInbox = useTradeReviewInbox(!!episodeId);
  const plans = usePositionCalculatorPlans();
  const playbooks = usePlaybooks();
  const tagCategories = useTagCategories();
  const queryClient = useQueryClient();
  const fetcher = useGraphQL();
  const [error, setError] = useState("");
  const [tagIdsByCategory, setTagIdsByCategory] = useState<
    Record<string, string[]>
  >({});
  const formId = React.useId();
  const fieldIds = {
    symbol: `${formId}-symbol`,
    symbolName: `${formId}-symbol-name`,
    openDate: `${formId}-open-date`,
    closeDate: `${formId}-close-date`,
    entryPrice: `${formId}-entry-price`,
    exitPrice: `${formId}-exit-price`,
    positionSize: `${formId}-position-size`,
    stopLoss: `${formId}-stop-loss`,
    lesson: `${formId}-lesson`,
    notes: `${formId}-notes`,
  };

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
    ((prefillQuery.isLoading && selectedTransactions.length === 0) ||
      (!!episodeId && reviewInbox.isLoading));

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
    stopLossMode: "set",
    tradeType: defaults.tradeType,
    playbookId: "",
    notes: "",
    violatedPrincipleIds: [],
    planId: "",
    planAdherence: "",
    lesson: "",
  });

  const reviewItem = (reviewInbox.data ?? []).find(
    (item) => item.episodeId === episodeId,
  );
  const suggestions = useMemo(
    () => parseSuggestions(reviewItem?.suggestionsJson),
    [reviewItem?.suggestionsJson],
  );
  const eligiblePlanIds = new Set([
    ...suggestions.map((suggestion) => suggestion.planId),
    ...(reviewItem?.confirmedPlanId ? [reviewItem.confirmedPlanId] : []),
  ]);
  const eligiblePlans = (plans.data ?? []).filter((plan) =>
    eligiblePlanIds.has(plan.id),
  );
  const selectedPlan = eligiblePlans.find((plan) => plan.id === form.planId);

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
    if (episodeId && reviewInbox.isLoading) return;
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
      stopLossMode: "set",
      tradeType: d.tradeType,
      playbookId: "",
      notes: "",
      violatedPrincipleIds: [],
      planId: reviewItem?.confirmedPlanId ?? suggestions.at(0)?.planId ?? "",
      planAdherence: "",
      lesson: "",
    });
    setTagIdsByCategory({});
    setError("");
    seededRef.current = true;
  }, [
    open,
    selectedTransactions,
    episodeId,
    reviewInbox.isLoading,
    reviewItem?.confirmedPlanId,
    suggestions,
  ]);

  // Principles are account-scoped. One from the previous workspace is not a valid
  // violation for this trade, and the backend rejects the whole create.
  const workspaceId = account?.id ?? null;
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset only on account change
  React.useEffect(() => {
    setForm((c) => ({ ...c, violatedPrincipleIds: [] }));
  }, [workspaceId]);

  // Changing playbook drops principles scoped to the old playbook; account-wide
  // ones survive.
  const principlesQuery = usePrinciples(workspaceId);
  const selectedPlaybookId = form.playbookId || null;
  // biome-ignore lint/correctness/useExhaustiveDependencies: prune only on playbook change
  React.useEffect(() => {
    const byId = new Map((principlesQuery.data ?? []).map((p) => [p.id, p]));
    setForm((current) => ({
      ...current,
      violatedPrincipleIds: current.violatedPrincipleIds.filter((id) => {
        const p = byId.get(id);
        return p
          ? p.playbookId === null || p.playbookId === selectedPlaybookId
          : false;
      }),
    }));
  }, [selectedPlaybookId]);

  function setField<K extends keyof MergeFormState>(
    key: K,
    value: MergeFormState[K],
  ) {
    setForm((c) => ({ ...c, [key]: value }));
    if (error) setError("");
  }

  function validate(): string {
    if (!account) return "No active workspace";
    if (episodeId) {
      if (!form.planId && form.stopLossMode === "set" && !form.stopLoss.trim())
        return "Enter the trade's stop loss or choose No stop loss";
      return "";
    }
    if (!form.symbol.trim()) return "Symbol is required";
    if (!form.openDate) return "Open date is required";
    if (!form.closeDate) return "Close date is required";
    if (!form.entryPrice.trim()) return "Entry price is required";
    if (!form.exitPrice.trim()) return "Exit price is required";
    if (!form.positionSize.trim()) return "Position size is required";
    if (form.stopLossMode === "set" && !form.stopLoss.trim())
      return "Enter a stop loss price or choose No stop loss";
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

    const tagIds = Object.values(tagIdsByCategory).flat();

    try {
      if (episodeId) {
        await publishEpisode.mutateAsync({
          episodeId,
          planId: form.planId || null,
          stopLoss:
            form.planId || form.stopLossMode === "none"
              ? null
              : Number(form.stopLoss),
          playbookId: form.playbookId || null,
          notes: form.notes.trim() || null,
          planAdherence: form.planId
            ? form.planAdherence || null
            : "No position plan",
          lesson: form.lesson.trim() || null,
          tagIds,
          violatedPrincipleIds: form.violatedPrincipleIds,
        });
        capture(EVENTS.tradesMerged, { count: selectedTransactions.length });
        setOpen(false);
        onSuccess();
        return;
      }
      await createTrade.mutateAsync({
        workspaceId: account.id,
        symbol: form.symbol.trim().toUpperCase(),
        symbolName: form.symbolName.trim() || undefined,
        openDate: form.openDate,
        closeDate: form.closeDate,
        entryPrice: Number(form.entryPrice),
        exitPrice: Number(form.exitPrice),
        positionSize: Number(form.positionSize),
        stopLoss: form.stopLossMode === "none" ? 0 : Number(form.stopLoss),
        tradeType: form.tradeType,
        tagIds,
        violatedPrincipleIds: form.violatedPrincipleIds,
        playbookId: form.playbookId || undefined,
        notes: form.notes.trim() || undefined,
        brokerageTransactionIds: selectedTransactions.map((t) => t.id),
        contractMultiplier: defaults.contractMultiplier,
      });
      queryClient.invalidateQueries({ queryKey: ["linked-brokerage-tx-ids"] });
      capture(EVENTS.tradesMerged, { count: selectedTransactions.length });
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
              <DialogTitle>
                {episodeId ? "Review broker trade" : "Merge Trades to Journal"}
              </DialogTitle>
              <DialogDescription>
                {episodeId
                  ? `Check the broker execution and add the context only you know.`
                  : `Merging ${selectedTransactions.length} ${defaults.symbol} trades into a journal entry.`}
                {defaults.isOption
                  ? ` Option contract (×${defaults.contractMultiplier}) — ${defaults.symbolName || defaults.symbol}.`
                  : ""}
              </DialogDescription>
            </DialogHeader>

            {/* Selected trades summary */}
            <div
              className={cn(
                "my-4 rounded-lg border bg-muted/30 p-3",
                episodeId && "border-l-2 border-l-sky-500",
              )}
            >
              <p className="mb-2 text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground">
                {episodeId ? "Broker record · locked" : "Selected trades"}
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
                        className={`w-10 font-medium ${t.transactionType === "BUY" ? "text-emerald-600 dark:text-emerald-400" : "text-rose-600 dark:text-rose-400"}`}
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
              <Field label="Symbol" htmlFor={fieldIds.symbol}>
                <Input
                  id={fieldIds.symbol}
                  value={form.symbol}
                  onChange={(e) => setField("symbol", e.target.value)}
                  disabled={!!episodeId}
                />
              </Field>
              <Field label="Symbol Name" htmlFor={fieldIds.symbolName}>
                <Input
                  id={fieldIds.symbolName}
                  value={form.symbolName}
                  onChange={(e) => setField("symbolName", e.target.value)}
                  placeholder="Optional, auto-fetched"
                  disabled={!!episodeId}
                />
              </Field>
              <Field label="Open Date" htmlFor={fieldIds.openDate}>
                <DateTimePicker
                  id={fieldIds.openDate}
                  value={form.openDate}
                  onChange={(value) => setField("openDate", value)}
                  disabled={!!episodeId}
                />
              </Field>
              <Field label="Close Date" htmlFor={fieldIds.closeDate}>
                <DateTimePicker
                  id={fieldIds.closeDate}
                  value={form.closeDate}
                  onChange={(value) => setField("closeDate", value)}
                  disabled={!!episodeId}
                />
              </Field>
              <Field label="Entry Price" htmlFor={fieldIds.entryPrice}>
                <Input
                  id={fieldIds.entryPrice}
                  inputMode="decimal"
                  value={form.entryPrice}
                  onChange={(e) => setField("entryPrice", e.target.value)}
                  placeholder="0.00"
                  disabled={!!episodeId}
                />
              </Field>
              <Field label="Exit Price" htmlFor={fieldIds.exitPrice}>
                <Input
                  id={fieldIds.exitPrice}
                  inputMode="decimal"
                  value={form.exitPrice}
                  onChange={(e) => setField("exitPrice", e.target.value)}
                  placeholder="0.00"
                  disabled={!!episodeId}
                />
              </Field>
              <Field
                label={
                  defaults.isOption
                    ? "Position Size (contracts)"
                    : "Position Size"
                }
                htmlFor={fieldIds.positionSize}
              >
                <Input
                  id={fieldIds.positionSize}
                  inputMode="decimal"
                  value={form.positionSize}
                  onChange={(e) => setField("positionSize", e.target.value)}
                  placeholder="0.00"
                  disabled={!!episodeId}
                />
              </Field>
              <Field label="Stop Loss" htmlFor={fieldIds.stopLoss}>
                {selectedPlan ? (
                  <div className="flex h-9 items-center rounded-md border bg-muted/40 px-3 text-sm tabular-nums text-muted-foreground">
                    ${selectedPlan.stopLoss.toFixed(2)} · from matched plan
                  </div>
                ) : (
                  <div className="grid grid-cols-2 gap-2">
                    <Select
                      value={form.stopLossMode}
                      onValueChange={(v) =>
                        setField("stopLossMode", v as "set" | "none")
                      }
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="set">Stop loss</SelectItem>
                        <SelectItem value="none">No stop loss</SelectItem>
                      </SelectContent>
                    </Select>
                    {form.stopLossMode === "set" ? (
                      <Input
                        id={fieldIds.stopLoss}
                        inputMode="decimal"
                        value={form.stopLoss}
                        onChange={(e) => setField("stopLoss", e.target.value)}
                        placeholder="Price"
                      />
                    ) : (
                      <span className="flex items-center px-1 text-xs text-muted-foreground">
                        No stop recorded
                      </span>
                    )}
                  </div>
                )}
              </Field>
              <Field label="Trade Type">
                <Select
                  value={form.tradeType}
                  onValueChange={(v) => setField("tradeType", v as TradeType)}
                  disabled={!!episodeId}
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
              {episodeId ? (
                <>
                  <Field label="Position plan">
                    <Select
                      value={form.planId || "__none__"}
                      onValueChange={(value) =>
                        setField("planId", value === "__none__" ? "" : value)
                      }
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="No matching plan" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="__none__">
                          No matching plan
                        </SelectItem>
                        {eligiblePlans.map((plan) => (
                          <SelectItem key={plan.id} value={plan.id}>
                            {plan.symbol} · {plan.positionType} · $
                            {plan.stopLoss.toFixed(2)} stop
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <p className="text-[0.6875rem] text-muted-foreground">
                      {reviewItem?.confirmedPlanId
                        ? "Confirmed match. Choose another eligible plan to correct it."
                        : suggestions.length > 0
                          ? "Suggested from symbol, direction, size, and execution time."
                          : "No eligible saved plan was found."}
                    </p>
                  </Field>
                  {form.planId ? (
                    <Field label="Plan adherence">
                      <Select
                        value={form.planAdherence || "__unset__"}
                        onValueChange={(value) =>
                          setField(
                            "planAdherence",
                            value === "__unset__" ? "" : value,
                          )
                        }
                      >
                        <SelectTrigger>
                          <SelectValue placeholder="Choose one" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="__unset__">
                            Not answered
                          </SelectItem>
                          <SelectItem value="Followed">
                            Followed the plan
                          </SelectItem>
                          <SelectItem value="Partially followed">
                            Partially followed
                          </SelectItem>
                          <SelectItem value="Deviated">
                            Deviated from the plan
                          </SelectItem>
                        </SelectContent>
                      </Select>
                    </Field>
                  ) : null}
                  <Field
                    label="Lesson"
                    htmlFor={fieldIds.lesson}
                    className="md:col-span-2"
                  >
                    <textarea
                      id={fieldIds.lesson}
                      value={form.lesson}
                      onChange={(event) =>
                        setField("lesson", event.target.value)
                      }
                      placeholder="What will you repeat or change next time?"
                      rows={3}
                      className="min-h-20 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30"
                    />
                  </Field>
                </>
              ) : null}
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
              <Field
                label={episodeId ? "Additional context" : "Notes"}
                htmlFor={fieldIds.notes}
              >
                <textarea
                  id={fieldIds.notes}
                  value={form.notes}
                  onChange={(e) => setField("notes", e.target.value)}
                  placeholder={
                    episodeId
                      ? "What influenced the execution?"
                      : "Optional notes"
                  }
                  rows={4}
                  className={cn(
                    "min-h-24 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30",
                  )}
                />
              </Field>
              <Field label="Principles broken" className="self-start">
                <PrinciplePicker
                  workspaceId={account?.id ?? null}
                  selectedPlaybookId={selectedPlaybookId}
                  value={form.violatedPrincipleIds}
                  onChange={(ids) => setField("violatedPrincipleIds", ids)}
                />
              </Field>
            </div>

            {/* Per-category tag pickers */}
            {(tagCategories.data ?? []).length > 0 && (
              <div className="grid gap-4 pb-4 md:grid-cols-2">
                {(tagCategories.data ?? []).map((category) => (
                  <Field key={category.id} label={category.name}>
                    <TagPicker
                      category={category}
                      selectedTagIds={tagIdsByCategory[category.id] ?? []}
                      onChange={(ids) =>
                        setTagIdsByCategory((prev) => ({
                          ...prev,
                          [category.id]: ids,
                        }))
                      }
                    />
                  </Field>
                ))}
              </div>
            )}

            {error && <p className="pb-3 text-sm text-destructive">{error}</p>}

            <DialogFooter>
              <Button
                type="submit"
                disabled={createTrade.isPending || publishEpisode.isPending}
              >
                {createTrade.isPending || publishEpisode.isPending
                  ? "Publishing..."
                  : episodeId
                    ? "Publish to Journal"
                    : "Create Journal Entry"}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
