"use client";

import {
  ArrowDown01Icon,
  Calculator01Icon,
  Cancel01Icon,
  CheckmarkCircle01Icon,
  Delete02Icon,
  PlusSignIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@tradstry/app-ui/components/ui/dialog";
import { Input } from "@tradstry/app-ui/components/ui/input";
import { Label } from "@tradstry/app-ui/components/ui/label";
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import { Separator } from "@tradstry/app-ui/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@tradstry/app-ui/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@tradstry/app-ui/components/ui/toggle-group";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces/hooks";
import {
  useCreatePositionCalculatorHistory,
  useCreatePositionCalculatorPlan,
  useDeletePositionCalculatorHistory,
  useDeletePositionCalculatorPlan,
  useDismissManualExecution,
  useManualExecutionClaims,
  usePositionCalculatorHistory,
  usePositionCalculatorPlans,
  usePositionCalculatorRule,
  useTradeReviewInbox,
  useRequestPlanExecutionCheck,
  useConfirmTradeMatch,
  useFinalizeTradeReview,
  usePublishTradeReview,
  useRecordManualExecution,
  useUpdatePositionCalculatorPlan,
  useUpsertPositionCalculatorRule,
} from "@tradstry/app-ui/hooks/position-calculator";
import { usePrinciples } from "@tradstry/app-ui/hooks/principle";
import {
  calculateRiskBudget,
  calculateTrancheRisk,
  summarizePlanRisk,
} from "@tradstry/app-ui/lib/position-calculator-risk";
import {
  resolveHistoryTranches,
  trancheRisk,
} from "@tradstry/app-ui/lib/position-calculator-history";
import type {
  ManualExecutionClaim,
  PositionCalculatorHistoryEntry,
  PositionCalculatorPlan,
  TradeReviewInboxItem,
  TradeReviewMatchSuggestion,
} from "@tradstry/app-ui/lib/types/position-calculator";
import { cn } from "@tradstry/app-ui/lib/utils";
import * as React from "react";
import { toast } from "sonner";

type PositionType = "long" | "short";

type FormState = {
  symbol: string;
  entryPrice: string;
  stopLoss: string;
  accountBalance: string;
  accountRisk: string;
};

const initialForm: FormState = {
  symbol: "",
  entryPrice: "",
  stopLoss: "",
  accountBalance: "",
  accountRisk: "",
};

function localDateTimeInputValue(date = new Date()) {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000)
    .toISOString()
    .slice(0, 16);
}

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

function ResultRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between py-1">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="text-sm font-semibold tabular-nums">{value}</span>
    </div>
  );
}

function getStopLossError(
  entryPrice: string,
  stopLoss: string,
  positionType: PositionType,
): string | null {
  const entry = parseFloat(entryPrice);
  const stop = parseFloat(stopLoss);
  if (!Number.isFinite(entry) || !Number.isFinite(stop)) return null;
  if (positionType === "long" && stop >= entry)
    return "Stop loss must be below entry price for a long position.";
  if (positionType === "short" && stop <= entry)
    return "Stop loss must be above entry price for a short position.";
  return null;
}

function calculate(form: FormState, positionType: PositionType) {
  const entry = parseFloat(form.entryPrice);
  const stop = parseFloat(form.stopLoss);
  const balance = parseFloat(form.accountBalance);
  const risk = parseFloat(form.accountRisk);

  if (
    !Number.isFinite(entry) ||
    !Number.isFinite(stop) ||
    !Number.isFinite(balance) ||
    !Number.isFinite(risk) ||
    balance <= 0 ||
    risk <= 0 ||
    entry <= 0 ||
    stop <= 0
  )
    return null;

  if (positionType === "long" && stop >= entry) return null;
  if (positionType === "short" && stop <= entry) return null;

  const stopDistance = Math.abs(entry - stop);
  if (stopDistance === 0) return null;

  const riskAmount = balance * (risk / 100);
  const shares = riskAmount / stopDistance;
  const dollarValue = shares * entry;
  const accountPct = (dollarValue / balance) * 100;
  const stopLossPct = (stopDistance / entry) * 100;

  return { shares, dollarValue, accountPct, stopLossPct, riskAmount };
}

function fmt(n: number, decimals = 2) {
  return n.toLocaleString("en-US", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

function planInstrument(plan: PositionCalculatorPlan): {
  isOption: boolean;
  multiplier: number;
} {
  const instrument = parseJson<{
    asset_class?: string;
    multiplier?: string;
  } | null>(plan.instrumentJson, null);
  const multiplier = Number(instrument?.multiplier);
  return {
    isOption: instrument?.asset_class === "option",
    multiplier:
      instrument?.asset_class === "option" &&
      Number.isFinite(multiplier) &&
      multiplier > 0
        ? multiplier
        : 1,
  };
}

// ---------------------------------------------------------------------------
// Calculator tab
// ---------------------------------------------------------------------------

function CalculatorTab({
  rule,
  onPlan,
}: {
  rule: {
    accountBalance: number;
    accountRisk: number;
    maxStopLossPct: number;
  } | null;
  onPlan: (data: {
    symbol: string;
    positionType: PositionType;
    entryPrice: number;
    stopLoss: number;
    accountBalance: number;
    accountRisk: number;
    totalShares: number;
    positionValue: number;
  }) => void;
}) {
  const [form, setForm] = React.useState<FormState>(() => {
    if (typeof window === "undefined") return initialForm;
    try {
      const stored = localStorage.getItem("position-calculator");
      if (stored) return { ...initialForm, ...JSON.parse(stored) };
    } catch {}
    return initialForm;
  });
  const [positionType, setPositionType] = React.useState<PositionType>(() => {
    if (typeof window === "undefined") return "long";
    return (
      (localStorage.getItem("position-calculator-type") as PositionType) ||
      "long"
    );
  });
  const [roundedShares, setRoundedShares] = React.useState<number | null>(null);
  const createHistory = useCreatePositionCalculatorHistory();
  const activeWorkspace = useActiveWorkspace();
  const workspaceId = activeWorkspace?.id ?? null;
  const syncedBalance = activeWorkspace?.totalValue ?? null;
  const currencyCode =
    activeWorkspace?.totalValueCurrency ?? activeWorkspace?.currency ?? "USD";

  // Persist to localStorage on changes
  React.useEffect(() => {
    localStorage.setItem("position-calculator", JSON.stringify(form));
  }, [form]);

  React.useEffect(() => {
    localStorage.setItem("position-calculator-type", positionType);
  }, [positionType]);

  // Refill from the newly-selected workspace. A balance typed for the previous
  // account is not a balance for this one, and `rule` is per-account now, so a
  // 10% paper-account risk must not follow you onto the main portfolio.
  // One setForm, so a typed accountRisk is never clobbered by a second write.
  // biome-ignore lint/correctness/useExhaustiveDependencies: refill only when the account or its rule changes
  React.useEffect(() => {
    const nextBalance = syncedBalance ?? rule?.accountBalance ?? null;
    setForm((current) => ({
      ...current,
      accountBalance:
        nextBalance != null ? String(nextBalance) : current.accountBalance,
      // No rule saved for this account -> keep what the user typed. Blanking
      // it kills the whole results panel (calculate() needs a finite risk).
      accountRisk:
        rule?.accountRisk != null
          ? String(rule.accountRisk)
          : current.accountRisk,
    }));
    setRoundedShares(null);
  }, [workspaceId, syncedBalance, rule?.accountBalance, rule?.accountRisk]);

  // Reset rounding choice when inputs change
  function setField<K extends keyof FormState>(key: K, value: string) {
    setForm((current) => ({ ...current, [key]: value }));
    setRoundedShares(null);
  }

  const stopLossError = getStopLossError(
    form.entryPrice,
    form.stopLoss,
    positionType,
  );
  const result = calculate(form, positionType);
  const stopLossWarning =
    result && rule && result.stopLossPct > rule.maxStopLossPct
      ? `Stop loss distance (${fmt(result.stopLossPct)}%) exceeds your rule maximum of ${fmt(rule.maxStopLossPct)}%.`
      : null;

  async function handleSave() {
    if (!result) return;
    const entry = parseFloat(form.entryPrice);
    const balance = parseFloat(form.accountBalance);
    const finalShares = roundedShares ?? result.shares;
    const finalValue = finalShares * entry;
    const finalPct = (finalValue / balance) * 100;

    const toastId = toast.loading("Saving to history...");
    try {
      await createHistory.mutateAsync({
        symbol: form.symbol.trim() || "—",
        positionType,
        entryPrice: entry,
        stopLoss: parseFloat(form.stopLoss),
        accountBalance: balance,
        accountRisk: parseFloat(form.accountRisk),
        shares: finalShares,
        positionValue: finalValue,
        accountPct: finalPct,
        stopLossPct: result.stopLossPct,
      });
      toast.success("Saved to history.", { id: toastId });
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Failed to save to history.",
        { id: toastId },
      );
    }
  }

  return (
    <div className="grid gap-4 py-2">
      <div className="flex items-center justify-between">
        <Field label="Symbol" htmlFor="calc-symbol">
          <Input
            id="calc-symbol"
            value={form.symbol}
            onChange={(e) => setField("symbol", e.target.value)}
            placeholder="AAPL"
            className="w-40"
          />
        </Field>

        <div className="grid gap-2 self-end">
          <Label>Position Type</Label>
          <ToggleGroup
            type="single"
            variant="outline"
            value={positionType}
            onValueChange={(value) => {
              if (value) setPositionType(value as PositionType);
            }}
          >
            <ToggleGroupItem value="long" aria-label="Long">
              Long
            </ToggleGroupItem>
            <ToggleGroupItem value="short" aria-label="Short">
              Short
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <Field label="Entry Price" htmlFor="calc-entry">
          <Input
            id="calc-entry"
            type="number"
            step="0.0001"
            min="0"
            value={form.entryPrice}
            onChange={(e) => setField("entryPrice", e.target.value)}
            placeholder="0.00"
          />
        </Field>

        <Field label="Stop Loss" htmlFor="calc-stop">
          <Input
            id="calc-stop"
            type="number"
            step="0.0001"
            min="0"
            value={form.stopLoss}
            onChange={(e) => setField("stopLoss", e.target.value)}
            placeholder="0.00"
            className={
              stopLossError
                ? "border-destructive focus-visible:border-destructive"
                : ""
            }
          />
          {stopLossError ? (
            <p className="text-xs text-destructive">{stopLossError}</p>
          ) : null}
        </Field>

        <Field
          label={`Workspace Balance (${currencyCode})`}
          htmlFor="calc-balance"
        >
          <Input
            id="calc-balance"
            type="number"
            step="0.01"
            min="0"
            value={form.accountBalance}
            onChange={(e) => setField("accountBalance", e.target.value)}
            placeholder="10000.00"
          />
          {syncedBalance != null ? (
            String(syncedBalance) === form.accountBalance ? (
              <p className="text-xs text-muted-foreground">
                ↻ Synced from {activeWorkspace?.name}
              </p>
            ) : (
              <p className="text-xs text-muted-foreground">
                Overridden ·{" "}
                <button
                  type="button"
                  className="underline underline-offset-2 hover:text-foreground"
                  onClick={() =>
                    setField("accountBalance", String(syncedBalance))
                  }
                >
                  Reset to {fmt(syncedBalance)}
                </button>
              </p>
            )
          ) : (
            <p className="text-xs text-muted-foreground">
              Default from your Rule
            </p>
          )}
        </Field>

        <Field label="Workspace Risk (%)" htmlFor="calc-risk">
          <Input
            id="calc-risk"
            type="number"
            step="0.01"
            min="0"
            max="100"
            value={form.accountRisk}
            onChange={(e) => setField("accountRisk", e.target.value)}
            placeholder="1.00"
          />
        </Field>
      </div>

      {result ? (
        <>
          <Separator />
          {stopLossWarning ? (
            <p className="rounded-md bg-yellow-500/10 px-3 py-2 text-xs text-yellow-600 dark:text-yellow-400">
              {stopLossWarning}
            </p>
          ) : null}
          {(() => {
            const hasDecimals = result.shares % 1 !== 0;
            const finalShares = roundedShares ?? result.shares;
            const entry = parseFloat(form.entryPrice);
            const balance = parseFloat(form.accountBalance);
            const finalValue = finalShares * entry;
            const finalPct = (finalValue / balance) * 100;

            const stopDistance = Math.abs(
              parseFloat(form.entryPrice) - parseFloat(form.stopLoss),
            );
            // Actual risk, not planned. Rounding shares down lowers real risk,
            // and that is exactly when the planned figure would mislead.
            const actualRisk = finalShares * stopDistance;

            const overBalance = finalValue > balance;
            const roundsToZero = Math.floor(result.shares) === 0;

            return (
              <>
                {overBalance ? (
                  <p className="rounded-md bg-yellow-500/10 px-3 py-2 text-xs text-yellow-600 dark:text-yellow-400">
                    Position value (${fmt(finalValue)}) exceeds your account
                    balance (${fmt(balance)}).
                  </p>
                ) : null}
                {roundsToZero ? (
                  <p className="rounded-md bg-yellow-500/10 px-3 py-2 text-xs text-yellow-600 dark:text-yellow-400">
                    Rounding down gives 0 shares. Your risk budget is smaller
                    than one share&apos;s stop distance.
                  </p>
                ) : null}
                <div className="grid gap-1">
                  <ResultRow label="Shares (raw)" value={fmt(result.shares)} />
                  {hasDecimals ? (
                    <div className="flex items-center justify-between rounded-md bg-muted/60 px-3 py-2">
                      <span className="text-xs text-muted-foreground">
                        Round shares?
                      </span>
                      <div className="flex gap-1">
                        <Button
                          type="button"
                          size="sm"
                          variant={
                            roundedShares === Math.floor(result.shares)
                              ? "default"
                              : "outline"
                          }
                          className="h-6 px-2 text-xs"
                          onClick={() =>
                            setRoundedShares(Math.floor(result.shares))
                          }
                        >
                          Down ({Math.floor(result.shares)})
                        </Button>
                        <Button
                          type="button"
                          size="sm"
                          variant={
                            roundedShares === Math.ceil(result.shares)
                              ? "default"
                              : "outline"
                          }
                          className="h-6 px-2 text-xs"
                          onClick={() =>
                            setRoundedShares(Math.ceil(result.shares))
                          }
                        >
                          Up ({Math.ceil(result.shares)})
                        </Button>
                      </div>
                    </div>
                  ) : null}
                  {roundedShares !== null ? (
                    <ResultRow
                      label="Shares to buy"
                      value={fmt(finalShares, 0)}
                    />
                  ) : null}
                  <div className="flex items-center justify-between py-0.5">
                    <span className="text-sm text-muted-foreground">Risk</span>
                    <span className="text-sm font-medium tabular-nums">
                      ${fmt(actualRisk)}
                      <span className="px-2 text-muted-foreground">·</span>
                      {fmt(result.stopLossPct)}%
                    </span>
                  </div>
                  <ResultRow
                    label="Position value"
                    value={`$${fmt(finalValue)}`}
                  />
                  <div className="flex items-center justify-between py-0.5">
                    <span className="text-sm text-muted-foreground">
                      % of account
                    </span>
                    <span
                      className={cn(
                        "text-sm font-medium tabular-nums",
                        overBalance && "text-destructive",
                      )}
                    >
                      {fmt(finalPct)}%
                    </span>
                  </div>
                </div>
                <div className="flex justify-end gap-2">
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    onClick={handleSave}
                    disabled={createHistory.isPending}
                  >
                    {createHistory.isPending ? "Saving..." : "Save to history"}
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    onClick={() => {
                      const entry = parseFloat(form.entryPrice);
                      const balance = parseFloat(form.accountBalance);
                      const fs = roundedShares ?? result.shares;
                      onPlan({
                        symbol: form.symbol.trim() || "—",
                        positionType,
                        entryPrice: entry,
                        stopLoss: parseFloat(form.stopLoss),
                        accountBalance: balance,
                        accountRisk: parseFloat(form.accountRisk),
                        totalShares: fs,
                        positionValue: fs * entry,
                      });
                    }}
                  >
                    Plan this position
                  </Button>
                </div>
              </>
            );
          })()}
        </>
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// History tab
// ---------------------------------------------------------------------------

function HistoryTab() {
  const history = usePositionCalculatorHistory();
  const plans = usePositionCalculatorPlans();
  const deleteEntry = useDeletePositionCalculatorHistory();

  if (history.isLoading) {
    return (
      <div className="py-12 text-center text-sm text-muted-foreground">
        Loading...
      </div>
    );
  }

  if (!history.data || history.data.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <span className="text-sm text-muted-foreground">
          No calculations saved yet. Use "Save to history" in the Calculator
          tab.
        </span>
      </div>
    );
  }

  return (
    <ScrollArea className="max-h-[28rem] py-2">
      <div className="grid gap-2 pr-3">
        {history.data.map((entry) => (
          <HistoryCard
            key={entry.id}
            entry={entry}
            plans={plans.data ?? []}
            deleting={deleteEntry.isPending}
            onDelete={() =>
              deleteEntry.mutate(entry.id, {
                onSuccess: () => toast.success("History entry deleted."),
                onError: (error) =>
                  toast.error(
                    error instanceof Error
                      ? error.message
                      : "Failed to delete history entry.",
                  ),
              })
            }
          />
        ))}
      </div>
    </ScrollArea>
  );
}

function HistoryCard({
  entry,
  plans,
  deleting,
  onDelete,
}: {
  entry: PositionCalculatorHistoryEntry;
  plans: PositionCalculatorPlan[];
  deleting: boolean;
  onDelete: () => void;
}) {
  const [expanded, setExpanded] = React.useState(false);
  const tranches = resolveHistoryTranches(entry, plans);
  const hasDetails = tranches.length > 0;
  const filledCount = tranches.filter(
    (tranche) => tranche.status === "filled",
  ).length;
  const skippedCount = tranches.filter(
    (tranche) => tranche.status === "skipped",
  ).length;
  const actualRisk = tranches.reduce(
    (sum, tranche) =>
      sum + (trancheRisk(entry.positionType, entry.stopLoss, tranche) ?? 0),
    0,
  );
  const createdAt = new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  }).format(new Date(entry.createdAt));

  return (
    <article className="overflow-hidden rounded-lg border border-border bg-background">
      <div className="flex items-start gap-3 p-3">
        <button
          type="button"
          className={cn(
            "min-w-0 flex-1 text-left",
            hasDetails &&
              "rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/30",
          )}
          aria-expanded={hasDetails ? expanded : undefined}
          onClick={() => hasDetails && setExpanded((value) => !value)}
        >
          <div className="flex items-center gap-2">
            <span className="font-mono text-sm font-semibold tracking-wide">
              {entry.symbol}
            </span>
            <span className="text-xs capitalize text-muted-foreground">
              {entry.positionType}
            </span>
            <span className="rounded-full border border-border bg-muted/40 px-1.5 py-0.5 text-[0.625rem] text-muted-foreground">
              {hasDetails ? "Planned execution" : "Saved calculation"}
            </span>
            <span className="ml-auto text-[0.625rem] tabular-nums text-muted-foreground">
              {createdAt}
            </span>
          </div>

          <div className="mt-3 grid grid-cols-4 gap-3">
            <HistoryMetric label="Avg entry" value={`$${fmt(entry.entryPrice)}`} />
            <HistoryMetric label="Stop" value={`$${fmt(entry.stopLoss)}`} />
            <HistoryMetric label="Filled" value={`${fmt(entry.shares)} shares`} />
            <HistoryMetric
              label="Risk"
              value={
                hasDetails
                  ? `$${fmt(actualRisk)}`
                  : `${fmt(entry.accountRisk)}%`
              }
            />
          </div>

          <div className="mt-2 flex items-center gap-2 text-[0.6875rem] text-muted-foreground">
            <span>${fmt(entry.positionValue)} position</span>
            <span>·</span>
            <span>{fmt(entry.accountPct)}% of account</span>
            {hasDetails ? (
              <>
                <span>·</span>
                <span>
                  {filledCount} filled
                  {skippedCount > 0 ? ` · ${skippedCount} skipped` : ""}
                </span>
                <HugeiconsIcon
                  icon={ArrowDown01Icon}
                  strokeWidth={2}
                  className={cn(
                    "ml-auto size-3.5 transition-transform motion-reduce:transition-none",
                    expanded && "rotate-180",
                  )}
                />
              </>
            ) : null}
          </div>
        </button>

        <Button
          type="button"
          variant="ghost"
          size="icon"
          title={`Delete ${entry.symbol} history entry`}
          className="size-7 text-muted-foreground hover:text-destructive"
          disabled={deleting}
          onClick={onDelete}
        >
          <HugeiconsIcon
            icon={Delete02Icon}
            strokeWidth={2}
            className="size-4"
          />
        </Button>
      </div>

      {expanded && hasDetails ? (
        <div className="border-t border-border bg-muted/20 px-3 py-3">
          <div className="mb-2 flex items-center justify-between">
            <p className="text-[0.625rem] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
              Execution ladder
            </p>
            <p className="text-[0.625rem] tabular-nums text-muted-foreground">
              ${fmt(entry.accountBalance * (entry.accountRisk / 100))} risk
              budget
            </p>
          </div>
          <div className="grid gap-1.5">
            {tranches.map((tranche, index) => {
              const risk = trancheRisk(
                entry.positionType,
                entry.stopLoss,
                tranche,
              );
              const isFilled = tranche.status === "filled";
              return (
                <div
                  key={tranche.id}
                  className="grid grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-2 rounded-md border border-border/70 bg-background px-2 py-2"
                >
                  <span className="font-mono text-[0.625rem] text-muted-foreground">
                    {String(index + 1).padStart(2, "0")}
                  </span>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2 text-xs">
                      <span
                        className={cn(
                          "font-medium capitalize",
                          isFilled
                            ? "text-emerald-600 dark:text-emerald-400"
                            : "text-muted-foreground",
                        )}
                      >
                        {tranche.status}
                      </span>
                      <span className="text-muted-foreground">
                        {fmt(tranche.percent)}% risk allocation
                      </span>
                    </div>
                    <p className="mt-0.5 truncate text-[0.6875rem] tabular-nums text-muted-foreground">
                      {isFilled ? (
                        <>
                          {fmt(tranche.shares)} shares @ $
                          {fmt(tranche.targetPrice)} → ${fmt(entry.stopLoss)}
                        </>
                      ) : (
                        <>Entry ${fmt(tranche.targetPrice)} was not filled</>
                      )}
                    </p>
                  </div>
                  <div className="text-right tabular-nums">
                    <p className="text-xs font-medium">
                      {risk == null ? "—" : `$${fmt(risk)}`}
                    </p>
                    <p className="text-[0.625rem] text-muted-foreground">
                      risk
                    </p>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ) : null}
    </article>
  );
}

function HistoryMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <p className="text-[0.625rem] uppercase tracking-[0.06em] text-muted-foreground">
        {label}
      </p>
      <p className="mt-0.5 truncate text-xs font-medium tabular-nums">{value}</p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Rule tab
// ---------------------------------------------------------------------------

function RuleTab() {
  const activeWorkspace = useActiveWorkspace();
  const workspaceId = activeWorkspace?.id ?? null;
  const ruleQuery = usePositionCalculatorRule(workspaceId);
  const upsertRule = useUpsertPositionCalculatorRule(workspaceId ?? "");

  const [accountBalance, setAccountBalance] = React.useState("");
  const [accountRisk, setAccountRisk] = React.useState("");
  const [maxStopLossPct, setMaxStopLossPct] = React.useState("");
  const [saved, setSaved] = React.useState(false);

  // Clear the previous workspace's rule before the new one loads, so it never
  // lingers in the inputs. Declared before the prefill effect below: React
  // runs effects in declaration order, and reversed, the clear would always
  // win over a loaded rule.
  // biome-ignore lint/correctness/useExhaustiveDependencies: clear only on account change
  React.useEffect(() => {
    setAccountBalance("");
    setAccountRisk("");
    setMaxStopLossPct("");
  }, [workspaceId]);

  // Pre-fill form when existing rule loads
  React.useEffect(() => {
    if (ruleQuery.data) {
      setAccountBalance(ruleQuery.data.accountBalance.toString());
      setAccountRisk(ruleQuery.data.accountRisk.toString());
      setMaxStopLossPct(ruleQuery.data.maxStopLossPct.toString());
    }
  }, [ruleQuery.data]);

  if (!workspaceId) {
    return (
      <p className="py-6 text-center text-sm text-muted-foreground">
        Select a workspace to set its position-sizing rule.
      </p>
    );
  }

  async function handleSave() {
    const balance = parseFloat(accountBalance);
    const risk = parseFloat(accountRisk);
    const maxStop = parseFloat(maxStopLossPct);
    if (
      !Number.isFinite(balance) ||
      !Number.isFinite(risk) ||
      !Number.isFinite(maxStop)
    )
      return;

    const toastId = toast.loading("Saving rule...");
    try {
      await upsertRule.mutateAsync({
        // Narrowed by the `!workspaceId` early return above; TS doesn't carry
        // that narrowing across the closure boundary into this function.
        workspaceId: workspaceId as string,
        accountBalance: balance,
        accountRisk: risk,
        maxStopLossPct: maxStop,
      });
      toast.success(`Rule saved for ${activeWorkspace?.name ?? "workspace"}.`, {
        id: toastId,
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Failed to save rule.",
        { id: toastId },
      );
    }
  }

  return (
    <div className="grid gap-4 py-2">
      <div className="rounded-md border border-border bg-muted/40 p-3">
        <p className="text-sm font-medium">
          Auto-fill · {activeWorkspace?.name}
        </p>
        <p className="mt-1 text-sm text-muted-foreground">
          Workspace balance and risk will be pre-filled in the calculator each
          time you open it.
        </p>
      </div>

      <div className="rounded-md border border-border bg-muted/40 p-3">
        <p className="text-sm font-medium">Stop loss warning</p>
        <p className="mt-1 text-sm text-muted-foreground">
          If the calculated stop loss distance exceeds your maximum, the
          calculator will show a warning.
        </p>
      </div>

      <Separator />

      <div className="grid grid-cols-2 gap-4">
        <Field label="Default Workspace Balance ($)" htmlFor="rule-balance">
          <Input
            id="rule-balance"
            type="number"
            step="0.01"
            min="0"
            value={accountBalance}
            onChange={(e) => setAccountBalance(e.target.value)}
            placeholder="10000.00"
          />
        </Field>

        <Field label="Default Workspace Risk (%)" htmlFor="rule-risk">
          <Input
            id="rule-risk"
            type="number"
            step="0.01"
            min="0"
            max="100"
            value={accountRisk}
            onChange={(e) => setAccountRisk(e.target.value)}
            placeholder="1.00"
          />
        </Field>

        <Field label="Risk per Trade ($)" htmlFor="rule-risk-amount">
          <Input
            id="rule-risk-amount"
            type="text"
            readOnly
            disabled
            value={(() => {
              const b = parseFloat(accountBalance);
              const r = parseFloat(accountRisk);
              return Number.isFinite(b) && Number.isFinite(r) && b > 0 && r > 0
                ? `$${fmt(b * (r / 100))}`
                : "—";
            })()}
          />
        </Field>

        <Field label="Max Stop Loss Distance (%)" htmlFor="rule-max-stop">
          <Input
            id="rule-max-stop"
            type="number"
            step="0.01"
            min="0"
            value={maxStopLossPct}
            onChange={(e) => setMaxStopLossPct(e.target.value)}
            placeholder="2.00"
          />
        </Field>
      </div>

      {(() => {
        const balance = parseFloat(accountBalance);
        const risk = parseFloat(accountRisk);
        const maxStop = parseFloat(maxStopLossPct);
        const riskAmount =
          Number.isFinite(balance) &&
          Number.isFinite(risk) &&
          balance > 0 &&
          risk > 0
            ? balance * (risk / 100)
            : null;

        if (!riskAmount) return null;

        const lines = [
          `Workspace Balance: $${fmt(balance)}`,
          `Workspace Risk: ${fmt(risk)}%`,
          `Risk per Trade: $${fmt(riskAmount)}`,
        ];
        if (Number.isFinite(maxStop) && maxStop > 0) {
          lines.push(`Max Stop Loss Distance: ${fmt(maxStop)}%`);
        }

        return (
          <Field label="Summary">
            <textarea
              readOnly
              rows={lines.length}
              value={lines.join("\n")}
              className="w-full resize-none rounded-md border border-input bg-muted/40 px-3 py-2 text-sm tabular-nums text-muted-foreground outline-none"
            />
          </Field>
        );
      })()}

      <div className="flex items-center justify-end gap-2">
        {saved ? (
          <span className="text-xs text-muted-foreground">Saved</span>
        ) : null}
        <Button
          type="button"
          size="sm"
          onClick={handleSave}
          disabled={upsertRule.isPending}
        >
          {upsertRule.isPending ? "Saving..." : "Save rule"}
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Plans tab
// ---------------------------------------------------------------------------

type PlanSeed = {
  symbol: string;
  positionType: PositionType;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  totalShares: number;
  positionValue: number;
};

function createTranche(targetPrice: number, percent = "") {
  return {
    id: crypto.randomUUID(),
    percent,
    targetPrice: targetPrice.toString(),
  };
}

function CreatePlanForm({
  seed,
  onDone,
}: {
  seed: PlanSeed;
  onDone: () => void;
}) {
  const createPlan = useCreatePositionCalculatorPlan();
  const activeWorkspace = useActiveWorkspace();
  const stopLossId = React.useId();
  const [stopLoss, setStopLoss] = React.useState(seed.stopLoss.toString());
  const canPlanOptions =
    activeWorkspace?.assetClass === "options" ||
    activeWorkspace?.assetClass === "mixed";
  const [isOption, setIsOption] = React.useState(
    activeWorkspace?.assetClass === "options",
  );
  const [optionExpiration, setOptionExpiration] = React.useState("");
  const [optionStrike, setOptionStrike] = React.useState("");
  const [optionKind, setOptionKind] = React.useState<"call" | "put">("call");
  const [optionMultiplier, setOptionMultiplier] = React.useState("100");
  const [tranches, setTranches] = React.useState([
    createTranche(seed.entryPrice, "100"),
  ]);

  // The last tranche absorbs whatever the others leave, so the total is 100%
  // without hand-balancing. Editing the last one directly is still respected.
  function rebalanceLast(list: ReturnType<typeof createTranche>[]) {
    if (list.length === 0) return list;
    if (list.length === 1) {
      const only = list[0];
      return only ? [{ ...only, percent: "100" }] : list;
    }
    const others = list.slice(0, -1);
    const used = others.reduce(
      (sum, t) => sum + (parseFloat(t.percent) || 0),
      0,
    );
    const remainder = Math.max(0, Math.round((100 - used) * 100) / 100);
    const last = list[list.length - 1];
    return [...others, { ...last, percent: String(remainder) }];
  }

  function addTranche() {
    setTranches((prev) =>
      rebalanceLast([...prev, createTranche(seed.entryPrice)]),
    );
  }

  function removeTranche(trancheId: string) {
    setTranches((prev) =>
      rebalanceLast(prev.filter((tranche) => tranche.id !== trancheId)),
    );
  }

  function updateTranche(
    trancheId: string,
    field: "percent" | "targetPrice",
    value: string,
  ) {
    setTranches((prev) => {
      const next = prev.map((tranche) =>
        tranche.id === trancheId ? { ...tranche, [field]: value } : tranche,
      );
      const isLast = prev[prev.length - 1]?.id === trancheId;
      // Editing the last tranche's percent is a manual override; everything
      // else rebalances the last one to keep the total at 100.
      return field === "percent" && !isLast ? rebalanceLast(next) : next;
    });
  }

  const totalPercent = tranches.reduce(
    (sum, t) => sum + (parseFloat(t.percent) || 0),
    0,
  );
  const parsedStopLoss = parseFloat(stopLoss);
  const parsedOptionStrike = parseFloat(optionStrike);
  const parsedOptionMultiplier = parseFloat(optionMultiplier);
  const effectiveMultiplier =
    isOption &&
    Number.isFinite(parsedOptionMultiplier) &&
    parsedOptionMultiplier > 0
      ? parsedOptionMultiplier
      : 1;
  const riskBudget = calculateRiskBudget(seed.accountBalance, seed.accountRisk);
  const calculatedTranches = tranches.map((tranche) => {
    const targetPrice = parseFloat(tranche.targetPrice);
    const calculation =
      riskBudget == null
        ? null
        : calculateTrancheRisk({
            positionType: seed.positionType,
            entryPrice: targetPrice,
            stopLoss: parsedStopLoss,
            riskBudget: riskBudget / effectiveMultiplier,
            riskPercent: parseFloat(tranche.percent),
          });
    const multiplierAdjustedCalculation = calculation
      ? {
          ...calculation,
          riskPerShare: calculation.riskPerShare * effectiveMultiplier,
          actualRisk: calculation.actualRisk * effectiveMultiplier,
        }
      : null;
    return { ...tranche, targetPrice, calculation: multiplierAdjustedCalculation };
  });
  const stopLossError = calculatedTranches.some((tranche) => {
    if (!Number.isFinite(tranche.targetPrice)) return false;
    return seed.positionType === "short"
      ? parsedStopLoss <= tranche.targetPrice
      : parsedStopLoss >= tranche.targetPrice;
  })
    ? `Stop loss must be ${seed.positionType === "short" ? "above" : "below"} every planned entry.`
    : null;
  const summarizedTranches = calculatedTranches.flatMap((tranche) =>
    tranche.calculation
      ? [
          {
            shares: tranche.calculation.shares,
            targetPrice: tranche.targetPrice,
            actualRisk: tranche.calculation.actualRisk,
          },
        ]
      : [],
  );
  const planSummary =
    summarizedTranches.length === calculatedTranches.length
      ? summarizePlanRisk(summarizedTranches)
      : null;
  const totalPercentIsValid = Math.abs(totalPercent - 100) < 0.001;
  const isValid =
    totalPercentIsValid &&
    Number.isFinite(parsedStopLoss) &&
    parsedStopLoss > 0 &&
    !stopLossError &&
    (!isOption ||
      (optionExpiration.length > 0 &&
        Number.isFinite(parsedOptionStrike) &&
        parsedOptionStrike > 0 &&
        Number.isFinite(parsedOptionMultiplier) &&
        parsedOptionMultiplier > 0)) &&
    planSummary != null;

  async function handleCreate() {
    if (!isValid) return;
    const readyTranches = calculatedTranches.flatMap((tranche) =>
      tranche.calculation
        ? [
            {
              percent: parseFloat(tranche.percent),
              shares: tranche.calculation.shares,
              targetPrice: tranche.targetPrice,
            },
          ]
        : [],
    );
    if (readyTranches.length !== tranches.length || !planSummary) return;

    const toastId = toast.loading("Creating plan...");
    try {
      await createPlan.mutateAsync({
        ...seed,
        entryPrice: planSummary.weightedEntry,
        stopLoss: parsedStopLoss,
        totalShares: planSummary.totalShares,
        positionValue: planSummary.positionValue * effectiveMultiplier,
        tranches: readyTranches,
        instrumentJson: isOption
          ? JSON.stringify({
              asset_class: "option",
              underlying: seed.symbol.trim().toUpperCase(),
              expiration: optionExpiration,
              strike: parsedOptionStrike.toString(),
              option_kind: optionKind,
              multiplier: parsedOptionMultiplier.toString(),
            })
          : null,
      });
      toast.success(`${seed.symbol} plan created.`, { id: toastId });
      onDone();
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Failed to create plan.",
        { id: toastId },
      );
    }
  }

  return (
    <div className="grid gap-3 py-2">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium">
            {seed.symbol} <span className="capitalize">{seed.positionType}</span>
          </p>
          <p className="text-xs text-muted-foreground">
            Allocate the trade&apos;s risk across planned entries.
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className="h-6 px-2 text-xs"
          onClick={addTranche}
        >
          <HugeiconsIcon
            icon={PlusSignIcon}
            strokeWidth={2}
            className="mr-1 size-3"
          />
          Add tranche
        </Button>
      </div>

      {canPlanOptions ? (
        <div className="grid gap-2 rounded-md border border-border bg-muted/20 p-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-sm font-medium">Instrument</p>
              <p className="text-xs text-muted-foreground">
                Option plans store the exact single-leg contract for broker matching.
              </p>
            </div>
            <ToggleGroup
              type="single"
              value={isOption ? "option" : "stock"}
              onValueChange={(value) => value && setIsOption(value === "option")}
            >
              <ToggleGroupItem value="stock" className="h-7 px-2 text-xs">Stock</ToggleGroupItem>
              <ToggleGroupItem value="option" className="h-7 px-2 text-xs">Option</ToggleGroupItem>
            </ToggleGroup>
          </div>
          {isOption ? (
            <div className="grid grid-cols-4 gap-2">
              <Field label="Expiration">
                <Input type="date" value={optionExpiration} onChange={(event) => setOptionExpiration(event.target.value)} className="h-8" />
              </Field>
              <Field label="Strike">
                <Input type="number" min="0" step="0.01" value={optionStrike} onChange={(event) => setOptionStrike(event.target.value)} className="h-8" />
              </Field>
              <Field label="Type">
                <select value={optionKind} onChange={(event) => setOptionKind(event.target.value as "call" | "put")} className="h-8 rounded-md border border-input bg-background px-2 text-sm">
                  <option value="call">Call</option>
                  <option value="put">Put</option>
                </select>
              </Field>
              <Field label="Multiplier">
                <Input type="number" min="1" step="1" value={optionMultiplier} onChange={(event) => setOptionMultiplier(event.target.value)} className="h-8" />
              </Field>
            </div>
          ) : null}
        </div>
      ) : null}

      <div className="rounded-md border border-border bg-muted/30 p-3">
        <div className="grid grid-cols-[minmax(0,1fr)_7.5rem] items-start gap-4">
          <div>
            <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Risk guardrail
            </p>
            <p className="mt-1 text-sm">
              {riskBudget != null ? `$${fmt(riskBudget)}` : "—"} maximum loss
              <span className="px-1.5 text-muted-foreground">·</span>
              {fmt(seed.accountRisk)}% of ${fmt(seed.accountBalance)}
            </p>
          </div>
          <Field label="Stop loss" htmlFor={stopLossId}>
            <Input
              id={stopLossId}
              type="number"
              step="0.0001"
              min="0"
              value={stopLoss}
              onChange={(event) => setStopLoss(event.target.value)}
              className={cn(
                "h-8 tabular-nums",
                stopLossError && "border-destructive",
              )}
            />
          </Field>
        </div>
        {stopLossError ? (
          <p className="mt-2 text-xs text-destructive">{stopLossError}</p>
        ) : null}
        {planSummary ? (
          <div className="mt-3 grid grid-cols-3 gap-3 border-t border-border/70 pt-3 text-xs">
            <div>
              <p className="text-muted-foreground">Weighted entry</p>
              <p className="mt-0.5 font-medium tabular-nums">
                ${fmt(planSummary.weightedEntry)}
              </p>
            </div>
            <div>
              <p className="text-muted-foreground">Planned shares</p>
              <p className="mt-0.5 font-medium tabular-nums">
                {fmt(planSummary.totalShares)}
              </p>
            </div>
            <div>
              <p className="text-muted-foreground">Planned risk</p>
              <p className="mt-0.5 font-medium tabular-nums">
                ${fmt(planSummary.totalRisk)}
              </p>
            </div>
          </div>
        ) : null}
      </div>

      <div className="grid grid-cols-[5rem_7.5rem_minmax(0,1fr)_1.75rem] gap-2 px-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        <span>Risk</span>
        <span>Entry</span>
        <span>Entry → stop</span>
        <span />
      </div>

      {calculatedTranches.map((tranche, index) => {
        const calculation = tranche.calculation;
        return (
          <div
            key={tranche.id}
            className="grid grid-cols-[5rem_7.5rem_minmax(0,1fr)_1.75rem] items-center gap-2 rounded-md border border-border/70 p-2"
          >
            <div className="relative">
              <Input
                aria-label={`Tranche ${index + 1} risk allocation percentage`}
                type="number"
                step="1"
                min="0"
                max="100"
                value={tranche.percent}
                onChange={(event) =>
                  updateTranche(tranche.id, "percent", event.target.value)
                }
                className="h-8 pr-6 tabular-nums"
              />
              <span className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">
                %
              </span>
            </div>
            <Input
              aria-label={`Tranche ${index + 1} entry price`}
              type="number"
              step="0.01"
              min="0"
              value={tranche.targetPrice}
              onChange={(event) =>
                updateTranche(tranche.id, "targetPrice", event.target.value)
              }
              className={cn(
                "h-8 tabular-nums",
                !calculation && "border-destructive",
              )}
            />
            <div className="min-w-0 text-xs tabular-nums">
              {calculation ? (
                <>
                  <p className="truncate font-medium">
                    ${fmt(tranche.targetPrice)} → ${fmt(parsedStopLoss)}
                    <span className="px-1.5 text-muted-foreground">·</span>
                    ${fmt(calculation.riskPerShare)}/share
                  </p>
                  <p className="truncate text-muted-foreground">
                    {fmt(calculation.shares)} shares
                    <span className="px-1.5">·</span>$
                    {fmt(calculation.actualRisk)} risk
                  </p>
                </>
              ) : (
                <p className="text-destructive">Entry must stay beyond stop</p>
              )}
            </div>
            {tranches.length > 1 ? (
              <Button
                type="button"
                size="icon"
                variant="ghost"
                className="size-7 text-muted-foreground hover:text-destructive"
                onClick={() => removeTranche(tranche.id)}
                title={`Remove tranche ${index + 1}`}
              >
                <HugeiconsIcon
                  icon={Delete02Icon}
                  strokeWidth={2}
                  className="size-3.5"
                />
              </Button>
            ) : null}
          </div>
        );
      })}

      {!totalPercentIsValid ? (
        <p className="text-xs text-destructive">
          Risk allocations must total 100% (currently {fmt(totalPercent)}%)
        </p>
      ) : null}

      <div className="flex justify-end gap-2">
        <Button type="button" size="sm" variant="outline" onClick={onDone}>
          Cancel
        </Button>
        <Button
          type="button"
          size="sm"
          onClick={handleCreate}
          disabled={!isValid || createPlan.isPending}
        >
          {createPlan.isPending ? "Creating..." : "Create plan"}
        </Button>
      </div>
    </div>
  );
}

function PlanCard({
  plan,
  brokerageConnected,
  brokerMatchConfirmed,
  manualClaims,
}: {
  plan: PositionCalculatorPlan;
  brokerageConnected: boolean;
  brokerMatchConfirmed: boolean;
  manualClaims: ManualExecutionClaim[];
}) {
  const updatePlan = useUpdatePositionCalculatorPlan();
  const deletePlan = useDeletePositionCalculatorPlan();
  const createHistory = useCreatePositionCalculatorHistory();
  const requestExecutionCheck = useRequestPlanExecutionCheck();
  const recordManualExecution = useRecordManualExecution();
  const dismissManualExecution = useDismissManualExecution();
  const [executionRequested, setExecutionRequested] = React.useState(false);
  const [manualDraft, setManualDraft] = React.useState<{
    trancheId: string;
    quantity: string;
    price: string;
    executedAt: string;
  } | null>(null);
  const [editPrices, setEditPrices] = React.useState<Record<string, string>>(
    () => {
      const initial: Record<string, string> = {};
      for (const t of plan.tranches) {
        if (t.status === "planned") {
          initial[t.id] = t.targetPrice.toString();
        }
      }
      return initial;
    },
  );

  const [completing, setCompleting] = React.useState(false);

  const filledCount = plan.tranches.filter((t) => t.status === "filled").length;
  const reconciledManualCount = new Set(
    manualClaims
      .filter((claim) => claim.status === "reconciled")
      .map((claim) => claim.trancheId),
  ).size;
  const unverifiedCount = manualClaims.filter(
    (claim) => claim.status === "pending",
  ).length;
  const instrument = planInstrument(plan);
  const riskBudget =
    calculateRiskBudget(plan.accountBalance, plan.accountRisk) ?? 0;
  const displayedTranches = plan.tranches.map((tranche) => {
    const editedPrice = editPrices[tranche.id];
    const parsedEditedPrice = editedPrice ? parseFloat(editedPrice) : NaN;
    const targetPrice =
      tranche.status === "planned" && Number.isFinite(parsedEditedPrice)
        ? parsedEditedPrice
        : tranche.targetPrice;
    const rawCalculation = calculateTrancheRisk({
      positionType: plan.positionType,
      entryPrice: targetPrice,
      stopLoss: plan.stopLoss,
      riskBudget: riskBudget / instrument.multiplier,
      riskPercent: tranche.percent,
    });
    const calculation = rawCalculation
      ? {
          ...rawCalculation,
          riskPerShare:
            rawCalculation.riskPerShare * instrument.multiplier,
          actualRisk: rawCalculation.actualRisk * instrument.multiplier,
        }
      : null;
    const shares =
      tranche.status === "planned" && calculation
        ? calculation.shares
        : tranche.shares;
    const riskPerShare =
      plan.positionType === "short"
        ? plan.stopLoss - targetPrice
        : targetPrice - plan.stopLoss;

    return {
      ...tranche,
      targetPrice,
      shares,
      calculation,
      actualRisk:
        riskPerShare > 0
          ? shares * riskPerShare * instrument.multiplier
          : null,
    };
  });
  const summarizedDisplayedTranches = displayedTranches.flatMap((tranche) =>
    tranche.actualRisk != null
      ? [
          {
            shares: tranche.shares,
            targetPrice: tranche.targetPrice,
            actualRisk: tranche.actualRisk,
          },
        ]
      : [],
  );
  const displayedSummary =
    summarizedDisplayedTranches.length === displayedTranches.length
      ? summarizePlanRisk(summarizedDisplayedTranches)
      : null;

  function handlePriceBlur(trancheId: string) {
    const raw = editPrices[trancheId];
    const newPrice = parseFloat(raw);
    const tranche = plan.tranches.find((t) => t.id === trancheId);
    if (!tranche || !Number.isFinite(newPrice) || newPrice <= 0) return;
    const calculation = calculateTrancheRisk({
      positionType: plan.positionType,
      entryPrice: newPrice,
      stopLoss: plan.stopLoss,
      riskBudget,
      riskPercent: tranche.percent,
    });
    if (!calculation) {
      toast.error(
        plan.positionType === "short"
          ? "A short entry must be below the stop loss."
          : "A long entry must be above the stop loss.",
      );
      return;
    }
    if (
      newPrice === tranche.targetPrice &&
      calculation.shares === tranche.shares
    )
      return;
    updatePlan.mutate(
      {
        id: plan.id,
        input: {
          tranches: [
            {
              id: trancheId,
              targetPrice: newPrice,
              shares: calculation.shares,
            },
          ],
        },
      },
      {
        // Error-only: a success toast on every blur would be noise.
        onError: (error) =>
          toast.error(
            error instanceof Error
              ? error.message
              : "Failed to update target price.",
          ),
      },
    );
  }

  async function handleTrancheStatus(trancheId: string, status: string) {
    const selectedTranche = displayedTranches.find(
      (tranche) => tranche.id === trancheId,
    );
    if (!selectedTranche) return;
    if (status === "filled" && !selectedTranche.calculation) {
      toast.error("Fix the entry price before marking this tranche filled.");
      return;
    }

    // Build the next state of tranches after this update
    const nextTranches = displayedTranches.map((tranche) =>
      tranche.id === trancheId ? { ...tranche, status } : tranche,
    );
    const allResolved = nextTranches.every((t) => t.status !== "planned");
    const filledTranches = nextTranches.filter((t) => t.status === "filled");

    // If not all resolved yet, just update the tranche status
    if (!allResolved) {
      updatePlan.mutate(
        {
          id: plan.id,
          input: {
            tranches: [
              {
                id: trancheId,
                status,
                ...(status === "filled"
                  ? {
                      targetPrice: selectedTranche.targetPrice,
                      shares: selectedTranche.shares,
                    }
                  : {}),
              },
            ],
          },
        },
        {
          onSuccess: () =>
            toast.success(
              status === "filled" ? "Tranche filled." : "Tranche skipped.",
            ),
          onError: (error) =>
            toast.error(
              error instanceof Error
                ? error.message
                : "Failed to update tranche.",
            ),
        },
      );
      return;
    }

    // All tranches resolved — determine outcome
    setCompleting(true);
    const toastId = toast.loading("Resolving plan...");

    try {
      // If nothing was filled, cancel the plan
      if (filledTranches.length === 0) {
        await updatePlan.mutateAsync({
          id: plan.id,
          input: {
            tranches: [{ id: trancheId, status }],
            status: "cancelled",
          },
        });
        toast.success(`No tranches filled — ${plan.symbol} plan cancelled.`, {
          id: toastId,
        });
        return;
      }

      const resolvedTranches = filledTranches;

      // Calculate weighted average entry
      const totalFilledShares = resolvedTranches.reduce(
        (sum, t) => sum + t.shares,
        0,
      );
      const weightedEntry =
        resolvedTranches.reduce((sum, t) => sum + t.shares * t.targetPrice, 0) /
        totalFilledShares;
      const positionValue = totalFilledShares * weightedEntry;
      const accountPct = (positionValue / plan.accountBalance) * 100;
      const stopLossPct =
        (Math.abs(weightedEntry - plan.stopLoss) / weightedEntry) * 100;

      // 1. Update the last tranche status
      const resolvedPlan = await updatePlan.mutateAsync({
        id: plan.id,
        input: {
          tranches: [
            {
              id: trancheId,
              status,
              ...(status === "filled"
                ? {
                    targetPrice: selectedTranche.targetPrice,
                    shares: selectedTranche.shares,
                  }
                : {}),
            },
          ],
        },
      });

      // 2. Create history entry
      await createHistory.mutateAsync({
        symbol: plan.symbol,
        positionType: plan.positionType,
        entryPrice: weightedEntry,
        stopLoss: plan.stopLoss,
        accountBalance: plan.accountBalance,
        accountRisk: plan.accountRisk,
        shares: totalFilledShares,
        positionValue,
        accountPct,
        stopLossPct,
        planId: plan.id,
        tranches: resolvedPlan.tranches.map((tranche) => ({
          id: tranche.id,
          percent: tranche.percent,
          shares: tranche.shares,
          targetPrice: tranche.targetPrice,
          status: tranche.status,
          filledAt: tranche.filledAt,
        })),
      });

      // 3. Mark plan as completed
      await updatePlan.mutateAsync({
        id: plan.id,
        input: { status: "completed" },
      });
      toast.success(
        `${plan.symbol} plan completed — ${fmt(totalFilledShares, 0)} shares @ $${fmt(weightedEntry)}. Moved to History.`,
        { id: toastId },
      );
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Failed to resolve plan.",
        { id: toastId },
      );
    } finally {
      setCompleting(false);
    }
  }

  function handleCancel() {
    updatePlan.mutate(
      { id: plan.id, input: { status: "cancelled" } },
      {
        onSuccess: () => toast.success(`${plan.symbol} plan cancelled.`),
        onError: (error) =>
          toast.error(
            error instanceof Error ? error.message : "Failed to cancel plan.",
          ),
      },
    );
  }

  async function handleRecordManualExecution() {
    if (!manualDraft) return;
    const quantity = Number(manualDraft.quantity);
    const price = Number(manualDraft.price);
    const executedAt = new Date(manualDraft.executedAt);
    if (!Number.isFinite(quantity) || quantity <= 0) {
      toast.error("Enter a valid execution quantity.");
      return;
    }
    if (!Number.isFinite(price) || price <= 0) {
      toast.error("Enter a valid execution price.");
      return;
    }
    if (Number.isNaN(executedAt.getTime())) {
      toast.error("Enter a valid execution time.");
      return;
    }

    const toastId = toast.loading("Recording manual execution...");
    try {
      await recordManualExecution.mutateAsync({
        planId: plan.id,
        trancheId: manualDraft.trancheId,
        quantity: manualDraft.quantity,
        price: manualDraft.price,
        executedAt: executedAt.toISOString(),
      });
      setManualDraft(null);
      toast.success("Saved as Manual · unverified.", { id: toastId });
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : "Could not record the manual execution.",
        { id: toastId },
      );
    }
  }

  return (
    <div className="rounded-md border border-border p-3">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium">
            {plan.symbol}{" "}
            <span className="capitalize text-muted-foreground">
              {plan.positionType}
            </span>
            {instrument.isOption ? (
              <span className="ml-1 text-[0.6875rem] uppercase text-muted-foreground">
                option
              </span>
            ) : null}
          </p>
          <p className="text-xs text-muted-foreground">
            {fmt(displayedSummary?.totalShares ?? plan.totalShares)} {instrument.isOption ? "contracts" : "shares"}
            <span className="px-1.5">·</span>stop ${fmt(plan.stopLoss)}
            <span className="px-1.5">·</span>
            {displayedSummary
              ? `$${fmt(displayedSummary.totalRisk)} risk`
              : `${fmt(plan.accountRisk)}% risk`} —{" "}
            {brokerageConnected
              ? brokerMatchConfirmed
                ? "Broker execution matched"
                : "Waiting for execution"
              : `${filledCount}/${plan.tranches.length} filled`}
            {brokerageConnected && reconciledManualCount > 0 ? (
              <span className="text-green-600 dark:text-green-400">
                <span className="px-1.5 text-muted-foreground">·</span>
                {reconciledManualCount} manual reconciled
              </span>
            ) : null}
            {brokerageConnected && unverifiedCount > 0 ? (
              <span className="text-amber-700 dark:text-amber-400">
                <span className="px-1.5 text-muted-foreground">·</span>
                {unverifiedCount} manual unverified
              </span>
            ) : null}
          </p>
        </div>
        <div className="flex gap-1">
          {plan.status === "active" ? (
            <Button
              type="button"
              size="icon"
              variant="ghost"
              className="size-7 text-muted-foreground hover:text-yellow-600"
              onClick={handleCancel}
              disabled={updatePlan.isPending}
              title="Cancel plan"
            >
              <HugeiconsIcon
                icon={Cancel01Icon}
                strokeWidth={2}
                className="size-3.5"
              />
            </Button>
          ) : null}
          <Button
            type="button"
            size="icon"
            variant="ghost"
            className="size-7 text-muted-foreground hover:text-destructive"
            onClick={() =>
              deletePlan.mutate(plan.id, {
                onSuccess: () => toast.success(`${plan.symbol} plan deleted.`),
                onError: (error) =>
                  toast.error(
                    error instanceof Error
                      ? error.message
                      : "Failed to delete plan.",
                  ),
              })
            }
            disabled={deletePlan.isPending}
            title="Delete plan"
          >
            <HugeiconsIcon
              icon={Delete02Icon}
              strokeWidth={2}
              className="size-3.5"
            />
          </Button>
        </div>
      </div>

      {plan.status !== "active" ? (
        <p className="mt-2 text-xs font-medium capitalize text-muted-foreground">
          {plan.status}
        </p>
      ) : (
        <div className="mt-2 grid gap-1">
          {displayedTranches.map((tranche) => (
            <div
              key={tranche.id}
              className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded bg-muted/40 px-2 py-2"
            >
              <div className="min-w-0 text-xs">
                <div className="flex items-center gap-1">
                  <span className="font-medium">
                    {fmt(tranche.percent, 0)}% risk
                  </span>
                  <span className="text-muted-foreground">—</span>
                  <span className="text-muted-foreground">
                    {fmt(tranche.shares)} shares @
                  </span>
                  {tranche.status === "planned" ? (
                    <Input
                      type="number"
                      step="0.01"
                      min="0"
                      value={
                        editPrices[tranche.id] ?? tranche.targetPrice.toString()
                      }
                      onChange={(e) =>
                        setEditPrices((prev) => ({
                          ...prev,
                          [tranche.id]: e.target.value,
                        }))
                      }
                      onBlur={() => handlePriceBlur(tranche.id)}
                      className={cn(
                        "h-5 w-20 px-1 text-xs tabular-nums",
                        !tranche.calculation && "border-destructive",
                      )}
                    />
                  ) : (
                    <span className="text-muted-foreground">
                      ${fmt(tranche.targetPrice)}
                    </span>
                  )}
                </div>
                <p
                  className={cn(
                    "mt-1 truncate tabular-nums text-muted-foreground",
                    tranche.actualRisk == null && "text-destructive",
                  )}
                >
                  {tranche.actualRisk != null && tranche.calculation ? (
                    <>
                      ${fmt(tranche.targetPrice)} → ${fmt(plan.stopLoss)}
                      <span className="px-1.5">·</span>$
                      {fmt(tranche.calculation.riskPerShare)}/share
                      <span className="px-1.5">·</span>$
                      {fmt(tranche.actualRisk)} risk
                    </>
                  ) : (
                    "Entry must stay beyond stop"
                  )}
                </p>
              </div>
              <div className="flex gap-1">
                {brokerageConnected && tranche.status === "planned" ? (
                  (() => {
                    const claim = manualClaims.find(
                      (candidate) => candidate.trancheId === tranche.id,
                    );
                    if (claim?.status === "reconciled") {
                      return (
                        <span className="text-[0.6875rem] font-medium text-green-600 dark:text-green-400">
                          Broker matched
                        </span>
                      );
                    }
                    if (claim) {
                      return (
                        <Button
                          type="button"
                          size="sm"
                          variant="ghost"
                          className="h-6 px-2 text-[0.6875rem] text-muted-foreground"
                          disabled={dismissManualExecution.isPending}
                          onClick={() =>
                            dismissManualExecution.mutate(claim.id, {
                              onSuccess: () =>
                                toast.success("Manual execution removed."),
                              onError: (error) =>
                                toast.error(
                                  error instanceof Error
                                    ? error.message
                                    : "Could not remove the manual execution.",
                                ),
                            })
                          }
                        >
                          Remove
                        </Button>
                      );
                    }
                    if (brokerMatchConfirmed) {
                      return (
                        <span className="text-[0.6875rem] font-medium text-green-600 dark:text-green-400">
                          Included in broker match
                        </span>
                      );
                    }
                    return (
                      <div className="flex items-center gap-1.5">
                        <span className="text-[0.6875rem] text-muted-foreground">
                          Waiting for execution
                        </span>
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          className="h-6 px-2 text-[0.6875rem]"
                          onClick={() =>
                            setManualDraft({
                              trancheId: tranche.id,
                              quantity: tranche.shares.toString(),
                              price: tranche.targetPrice.toString(),
                              executedAt: localDateTimeInputValue(),
                            })
                          }
                        >
                          Record manually
                        </Button>
                      </div>
                    );
                  })()
                ) : tranche.status === "planned" ? (
                  <>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      className="h-6 px-2 text-xs"
                      onClick={() => handleTrancheStatus(tranche.id, "filled")}
                      disabled={updatePlan.isPending || completing}
                    >
                      <HugeiconsIcon
                        icon={CheckmarkCircle01Icon}
                        strokeWidth={2}
                        className="mr-1 size-3"
                      />
                      Filled
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      className="h-6 px-2 text-xs text-muted-foreground"
                      onClick={() => handleTrancheStatus(tranche.id, "skipped")}
                      disabled={updatePlan.isPending || completing}
                    >
                      Skip
                    </Button>
                  </>
                ) : (
                  <span
                    className={`text-xs font-medium capitalize ${tranche.status === "filled" ? "text-green-600 dark:text-green-400" : "text-muted-foreground"}`}
                  >
                    {tranche.status}
                  </span>
                )}
              </div>
              {brokerageConnected &&
              manualClaims.find((claim) => claim.trancheId === tranche.id) ? (
                (() => {
                  const claim = manualClaims.find(
                    (candidate) => candidate.trancheId === tranche.id,
                  )!;
                  return (
                    <div
                      className={cn(
                        "col-span-2 mt-1 flex items-center justify-between gap-2 rounded border px-2.5 py-2 text-xs",
                        claim.status === "reconciled"
                          ? "border-green-200 bg-green-50/70 text-green-800 dark:border-green-900 dark:bg-green-950/30 dark:text-green-300"
                          : "border-amber-200 bg-amber-50/70 text-amber-800 dark:border-amber-900 dark:bg-amber-950/30 dark:text-amber-300",
                      )}
                    >
                      <span className="font-medium">
                        {claim.status === "reconciled"
                          ? "Broker matched"
                          : "Manual · unverified"}
                      </span>
                      <span className="tabular-nums">
                        {claim.quantity} shares @ ${fmt(Number(claim.price))}
                      </span>
                    </div>
                  );
                })()
              ) : null}
              {manualDraft?.trancheId === tranche.id ? (
                <div className="col-span-2 mt-1 grid gap-2 rounded border border-amber-200 bg-amber-50/70 p-2.5 dark:border-amber-900 dark:bg-amber-950/30">
                  <div className="grid grid-cols-3 gap-2">
                    <Field label="Quantity" htmlFor={`manual-quantity-${tranche.id}`}>
                      <Input
                        id={`manual-quantity-${tranche.id}`}
                        type="number"
                        min="0"
                        step="any"
                        className="h-7 bg-background text-xs"
                        value={manualDraft.quantity}
                        onChange={(event) =>
                          setManualDraft({
                            ...manualDraft,
                            quantity: event.target.value,
                          })
                        }
                      />
                    </Field>
                    <Field label="Price" htmlFor={`manual-price-${tranche.id}`}>
                      <Input
                        id={`manual-price-${tranche.id}`}
                        type="number"
                        min="0"
                        step="any"
                        className="h-7 bg-background text-xs"
                        value={manualDraft.price}
                        onChange={(event) =>
                          setManualDraft({
                            ...manualDraft,
                            price: event.target.value,
                          })
                        }
                      />
                    </Field>
                    <Field label="Executed at" htmlFor={`manual-time-${tranche.id}`}>
                      <Input
                        id={`manual-time-${tranche.id}`}
                        type="datetime-local"
                        className="h-7 bg-background text-xs"
                        value={manualDraft.executedAt}
                        onChange={(event) =>
                          setManualDraft({
                            ...manualDraft,
                            executedAt: event.target.value,
                          })
                        }
                      />
                    </Field>
                  </div>
                  <div className="flex items-center justify-between gap-3">
                    <p className="text-[0.6875rem] text-amber-800 dark:text-amber-300">
                      Saved as unverified until a broker match is confirmed.
                    </p>
                    <div className="flex gap-1.5">
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2 text-xs"
                        onClick={() => setManualDraft(null)}
                      >
                        Cancel
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        className="h-7 px-2 text-xs"
                        disabled={recordManualExecution.isPending}
                        onClick={handleRecordManualExecution}
                      >
                        {recordManualExecution.isPending
                          ? "Saving..."
                          : "Save manual execution"}
                      </Button>
                    </div>
                  </div>
                </div>
              ) : null}
            </div>
          ))}
          {brokerageConnected ? (
            <div className="mt-2 flex items-center justify-between gap-3 rounded-md border border-dashed border-border px-3 py-2">
              <p className="text-xs text-muted-foreground">
                Broker executions determine what actually filled. Tradstry will
                suggest a match for you to confirm.
              </p>
              <Button
                type="button"
                size="sm"
                className="shrink-0"
                disabled={requestExecutionCheck.isPending || executionRequested}
                onClick={async () => {
                  const toastId = toast.loading("Checking broker executions...");
                  try {
                    await requestExecutionCheck.mutateAsync(plan.id);
                    setExecutionRequested(true);
                    toast.success("Execution check started. Review any suggested match below.", { id: toastId });
                  } catch (error) {
                    toast.error(error instanceof Error ? error.message : "Could not check executions.", { id: toastId });
                  }
                }}
              >
                {requestExecutionCheck.isPending
                  ? "Checking..."
                  : executionRequested
                    ? "Check requested"
                    : "I entered this trade"}
              </Button>
            </div>
          ) : null}
        </div>
      )}
      {completing ? (
        <p className="mt-2 text-xs text-muted-foreground">Completing...</p>
      ) : null}
    </div>
  );
}

function parseJson<T>(value: string | null, fallback: T): T {
  if (!value) return fallback;
  try {
    return JSON.parse(value) as T;
  } catch {
    return fallback;
  }
}

function TradeReviewCard({
  item,
  plans,
}: {
  item: TradeReviewInboxItem;
  plans: PositionCalculatorPlan[];
}) {
  const confirmMatch = useConfirmTradeMatch();
  const finalizeReview = useFinalizeTradeReview();
  const publishReview = usePublishTradeReview();
  const activeWorkspace = useActiveWorkspace();
  const principles = usePrinciples(activeWorkspace?.id ?? null);
  const [reflection, setReflection] = React.useState("");
  const [noContext, setNoContext] = React.useState(false);
  const [principleResponses, setPrincipleResponses] = React.useState<
    Record<string, "followed" | "violated">
  >({});
  const suggestions = parseJson<TradeReviewMatchSuggestion[]>(
    item.suggestionsJson,
    [],
  );
  const latest = parseJson<{
    stage?: string;
    journalEntryId?: string | null;
    calculation?: {
      planned_quantity?: string;
      actual_quantity?: string;
      planned_weighted_entry?: string;
      actual_weighted_entry?: string;
      planned_risk?: string;
      actual_risk?: string;
      risk_drift?: string;
      flags?: string[];
    };
  } | null>(item.latestReviewJson, null);
  const calculation = latest?.calculation;
  const symbol = item.instrumentKey.replace(/^equity:/, "");

  return (
    <article className="rounded-md border border-border bg-muted/20 p-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-sm font-medium">
            {symbol} <span className="capitalize text-muted-foreground">{item.direction}</span>
          </p>
          <p className="mt-0.5 text-xs text-muted-foreground">
            Broker execution · {new Date(item.openedAt).toLocaleString()}
          </p>
        </div>
        <span className="rounded-full border px-2 py-0.5 text-[0.6875rem] capitalize text-muted-foreground">
          {latest?.stage === "final" ? "Reviewed" : item.matchStatus ?? "Needs match"}
        </span>
      </div>

      {!item.confirmedMatchId ? (
        <div className="mt-3 grid gap-2">
          {suggestions.length === 0 ? (
            <p className="text-xs text-muted-foreground">
              No eligible plan was found within the seven-day matching window.
            </p>
          ) : (
            suggestions.map((suggestion) => {
              const plan = plans.find((candidate) => candidate.id === suggestion.planId);
              return (
                <div key={suggestion.matchId} className="flex items-center justify-between gap-3 rounded bg-background px-2 py-2">
                  <div className="min-w-0 text-xs">
                    <p className="font-medium">Suggested: {plan?.symbol ?? suggestion.planId}</p>
                    <p className="text-muted-foreground">
                      Planned {suggestion.evidence.planned_quantity} · actual {suggestion.evidence.actual_quantity} shares
                    </p>
                  </div>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={confirmMatch.isPending}
                    onClick={async () => {
                      const toastId = toast.loading("Confirming match...");
                      try {
                        await confirmMatch.mutateAsync({ episodeId: item.episodeId, planId: suggestion.planId });
                        toast.success("Plan and broker execution matched.", { id: toastId });
                      } catch (error) {
                        toast.error(error instanceof Error ? error.message : "Could not confirm match.", { id: toastId });
                      }
                    }}
                  >
                    Confirm match
                  </Button>
                </div>
              );
            })
          )}
        </div>
      ) : calculation ? (
        <div className="mt-3 grid gap-3">
          <div className="grid grid-cols-3 gap-2 rounded bg-background p-2">
            <HistoryMetric label="Entry" value={`$${calculation.planned_weighted_entry ?? "—"} → $${calculation.actual_weighted_entry ?? "—"}`} />
            <HistoryMetric label="Size" value={`${calculation.planned_quantity ?? "—"} → ${calculation.actual_quantity ?? "—"}`} />
            <HistoryMetric label="Risk drift" value={`$${calculation.risk_drift ?? "—"}`} />
          </div>
          {(calculation.flags?.length ?? 0) > 0 ? (
            <div className="flex flex-wrap gap-1">
              {calculation.flags?.map((flag) => (
                <span key={flag} className="rounded-full bg-yellow-500/10 px-2 py-1 text-[0.6875rem] text-yellow-700 dark:text-yellow-300">
                  {flag.replaceAll("_", " ")}
                </span>
              ))}
            </div>
          ) : null}
          {latest.stage !== "final" ? (
            item.closedAt ? (
              <div className="grid gap-2">
              {(principles.data?.length ?? 0) > 0 ? (
                <div className="grid gap-1.5">
                  <Label>Principles</Label>
                  {principles.data?.filter((principle) => principle.isActive).map((principle) => (
                    <div key={principle.id} className="flex items-center justify-between gap-2 rounded border border-border bg-background px-2 py-1.5 text-xs">
                      <span className="min-w-0 truncate">{principle.title}</span>
                      <div className="flex gap-1">
                        {(["followed", "violated"] as const).map((response) => (
                          <Button
                            key={response}
                            type="button"
                            size="sm"
                            variant={principleResponses[principle.id] === response ? "default" : "ghost"}
                            className="h-6 px-2 text-[0.6875rem] capitalize"
                            onClick={() => setPrincipleResponses((current) =>
                              current[principle.id] === response
                                ? Object.fromEntries(Object.entries(current).filter(([id]) => id !== principle.id))
                                : { ...current, [principle.id]: response },
                            )}
                          >
                            {response}
                          </Button>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              ) : null}
              <Label htmlFor={`reflection-${item.episodeId}`}>What caused the deviation?</Label>
              <textarea
                id={`reflection-${item.episodeId}`}
                value={reflection}
                onChange={(event) => { setReflection(event.target.value); setNoContext(false); }}
                rows={2}
                placeholder="Record only what you know—Tradstry will not infer this."
                className="w-full resize-none rounded-md border border-input bg-background px-3 py-2 text-sm outline-none"
              />
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input type="checkbox" checked={noContext} onChange={(event) => { setNoContext(event.target.checked); if (event.target.checked) setReflection(""); }} />
                No additional context
              </label>
              <Button
                type="button"
                size="sm"
                className="justify-self-end"
                disabled={
                  finalizeReview.isPending ||
                  (!reflection.trim() &&
                    Object.keys(principleResponses).length === 0 &&
                    !noContext)
                }
                onClick={async () => {
                  if (!item.confirmedMatchId) return;
                  const toastId = toast.loading("Finalizing review...");
                  try {
                    await finalizeReview.mutateAsync({
                      matchId: item.confirmedMatchId,
                      reflectionJson: JSON.stringify({
                        ...(reflection.trim() ? { deviationReason: reflection.trim() } : {}),
                        followedPrincipleIds: Object.entries(principleResponses).filter(([, response]) => response === "followed").map(([id]) => id),
                        violatedPrincipleIds: Object.entries(principleResponses).filter(([, response]) => response === "violated").map(([id]) => id),
                      }),
                      noAdditionalContext: noContext,
                    });
                    toast.success("Trade review finalized as an immutable version.", { id: toastId });
                  } catch (error) {
                    toast.error(error instanceof Error ? error.message : "Could not finalize review.", { id: toastId });
                  }
                }}
              >
                Finalize review
              </Button>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">
                Entry review captured. The final reflection unlocks after the
                broker&apos;s closing fill syncs.
              </p>
            )
          ) : latest.journalEntryId ? (
            <p className="text-xs font-medium text-green-600 dark:text-green-400">
              Published to journal
            </p>
          ) : (
            <div className="flex items-center justify-between gap-3">
              <p className="text-xs text-muted-foreground">
                {item.closedAt
                  ? "Publish this broker-backed review to History."
                  : "The position is still open. Publish becomes available after the closing fill syncs."}
              </p>
              <Button
                type="button"
                size="sm"
                disabled={!item.closedAt || publishReview.isPending}
                onClick={async () => {
                  if (!item.confirmedMatchId) return;
                  const toastId = toast.loading("Publishing to journal...");
                  try {
                    await publishReview.mutateAsync(item.confirmedMatchId);
                    toast.success("Trade review published to History with its broker fills linked.", { id: toastId });
                  } catch (error) {
                    toast.error(error instanceof Error ? error.message : "Could not publish review.", { id: toastId });
                  }
                }}
              >
                Publish to History
              </Button>
            </div>
          )}
        </div>
      ) : null}
    </article>
  );
}

function PlansTab({
  seed,
  onClearSeed,
}: {
  seed: PlanSeed | null;
  onClearSeed: () => void;
}) {
  const plansQuery = usePositionCalculatorPlans();
  const activeWorkspace = useActiveWorkspace();
  const brokerageConnected = Boolean(
    activeWorkspace?.snaptradeAccountId &&
      !activeWorkspace.snaptradeConnectionDisabled,
  );
  const reviewInbox = useTradeReviewInbox(brokerageConnected);
  const manualClaims = useManualExecutionClaims(brokerageConnected);

  if (seed) {
    return <CreatePlanForm seed={seed} onDone={onClearSeed} />;
  }

  if (plansQuery.isLoading) {
    return (
      <div className="py-12 text-center text-sm text-muted-foreground">
        Loading...
      </div>
    );
  }

  // A completed plan has already produced its detailed History snapshot, so it
  // leaves this tab rather than lingering as a done card.
  const visiblePlans = (plansQuery.data ?? []).filter(
    (plan) => plan.status !== "completed",
  );

  if (visiblePlans.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <span className="text-sm text-muted-foreground">
          No open plans. Use "Plan this position" in the Calculator tab — filled
          plans land in History.
        </span>
      </div>
    );
  }

  const reviewItems = (reviewInbox.data ?? []).filter((item) => {
    const suggestions = parseJson<TradeReviewMatchSuggestion[]>(item.suggestionsJson, []);
    return Boolean(item.confirmedMatchId) || suggestions.length > 0;
  });

  return (
    <ScrollArea className="max-h-[26rem] py-2">
      <div className="grid gap-3 pr-3">
        {brokerageConnected && reviewItems.length > 0 ? (
          <section className="grid gap-2">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.06em] text-muted-foreground">Execution review</p>
              <p className="text-xs text-muted-foreground">Broker fills are the source of truth. Confirm every suggested match.</p>
            </div>
            {reviewItems.map((item) => (
              <TradeReviewCard key={item.episodeId} item={item} plans={plansQuery.data ?? []} />
            ))}
          </section>
        ) : null}
        {visiblePlans.map((plan) => (
          <PlanCard
            key={plan.id}
            plan={plan}
            brokerageConnected={brokerageConnected}
            brokerMatchConfirmed={reviewItems.some(
              (item) => item.confirmedPlanId === plan.id,
            )}
            manualClaims={(manualClaims.data ?? []).filter(
              (claim) => claim.planId === plan.id,
            )}
          />
        ))}
      </div>
    </ScrollArea>
  );
}

// ---------------------------------------------------------------------------
// Root modal
// ---------------------------------------------------------------------------

export function PositionCalculator({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const activeWorkspace = useActiveWorkspace();
  const ruleQuery = usePositionCalculatorRule(activeWorkspace?.id ?? null);
  const [activeTab, setActiveTab] = React.useState("calculator");
  const [planSeed, setPlanSeed] = React.useState<PlanSeed | null>(null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex min-h-[560px] max-h-[calc(100svh-2rem)] flex-col overflow-hidden sm:max-w-2xl">
        <DialogHeader className="shrink-0">
          <DialogTitle className="flex items-center gap-2">
            <span className="flex size-7 items-center justify-center rounded-md bg-muted text-muted-foreground">
              <HugeiconsIcon
                icon={Calculator01Icon}
                strokeWidth={2}
                className="size-4"
              />
            </span>
            Position Calculator
          </DialogTitle>
          <DialogDescription>
            Calculate your position size based on your account risk.
          </DialogDescription>
        </DialogHeader>

        <Tabs
          value={activeTab}
          onValueChange={setActiveTab}
          className="min-h-0 flex-1 flex-col overflow-hidden"
        >
          <TabsList className="shrink-0">
            <TabsTrigger value="calculator">Calculator</TabsTrigger>
            <TabsTrigger value="plans">Plans</TabsTrigger>
            <TabsTrigger value="history">History</TabsTrigger>
            <TabsTrigger value="rule">Rule</TabsTrigger>
          </TabsList>

          <TabsContent
            value="calculator"
            className="min-h-0 overflow-y-auto pr-1"
          >
            <CalculatorTab
              rule={ruleQuery.data ?? null}
              onPlan={(data) => {
                setPlanSeed(data);
                setActiveTab("plans");
              }}
            />
          </TabsContent>

          <TabsContent value="plans" className="min-h-0 overflow-y-auto pr-1">
            <PlansTab seed={planSeed} onClearSeed={() => setPlanSeed(null)} />
          </TabsContent>

          <TabsContent value="history" className="min-h-0 overflow-auto pr-1">
            <HistoryTab />
          </TabsContent>

          <TabsContent value="rule" className="min-h-0 overflow-y-auto pr-1">
            <RuleTab />
          </TabsContent>
        </Tabs>

        <div className="flex shrink-0 justify-end">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
          >
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
