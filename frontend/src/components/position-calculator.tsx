"use client";

import {
  Calculator01Icon,
  CheckmarkCircle01Icon,
  Delete02Icon,
  PlusSignIcon,
  Cancel01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import {
  useCreatePositionCalculatorHistory,
  useCreatePositionCalculatorPlan,
  useDeletePositionCalculatorHistory,
  useDeletePositionCalculatorPlan,
  usePositionCalculatorHistory,
  usePositionCalculatorPlans,
  usePositionCalculatorRule,
  useUpdatePositionCalculatorPlan,
  useUpsertPositionCalculatorRule,
} from "@/hooks/position-calculator";
import type { PositionCalculatorPlan } from "@/lib/types/position-calculator";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

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
  if (!isFinite(entry) || !isFinite(stop)) return null;
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
    !isFinite(entry) ||
    !isFinite(stop) ||
    !isFinite(balance) ||
    !isFinite(risk) ||
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

  return { shares, dollarValue, accountPct, stopLossPct };
}

function fmt(n: number, decimals = 2) {
  return n.toLocaleString("en-US", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

// ---------------------------------------------------------------------------
// Calculator tab
// ---------------------------------------------------------------------------

function CalculatorTab({
  rule,
  onPlan,
}: {
  rule: { accountBalance: number; accountRisk: number; maxStopLossPct: number } | null;
  onPlan: (data: { symbol: string; positionType: string; entryPrice: number; stopLoss: number; accountBalance: number; accountRisk: number; totalShares: number; positionValue: number }) => void;
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
    return (localStorage.getItem("position-calculator-type") as PositionType) || "long";
  });
  const [roundedShares, setRoundedShares] = React.useState<number | null>(null);
  const createHistory = useCreatePositionCalculatorHistory();

  // Persist to localStorage on changes
  React.useEffect(() => {
    localStorage.setItem("position-calculator", JSON.stringify(form));
  }, [form]);

  React.useEffect(() => {
    localStorage.setItem("position-calculator-type", positionType);
  }, [positionType]);

  // Auto-fill from rule on first load (only when form fields are still empty)
  React.useEffect(() => {
    if (rule) {
      setForm((current) => ({
        ...current,
        accountBalance: current.accountBalance || rule.accountBalance.toString(),
        accountRisk: current.accountRisk || rule.accountRisk.toString(),
      }));
    }
  }, [rule]);

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

        <Field label="Account Balance ($)" htmlFor="calc-balance">
          <Input
            id="calc-balance"
            type="number"
            step="0.01"
            min="0"
            value={form.accountBalance}
            onChange={(e) => setField("accountBalance", e.target.value)}
            placeholder="10000.00"
          />
        </Field>

        <Field label="Account Risk (%)" htmlFor="calc-risk">
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

            return (
              <>
                <div className="grid gap-1">
                  <ResultRow label="Shares (raw)" value={fmt(result.shares)} />
                  {hasDecimals ? (
                    <div className="flex items-center justify-between rounded-md bg-muted/60 px-3 py-2">
                      <span className="text-xs text-muted-foreground">
                        Round shares?
                      </span>
                      <div className="flex gap-1">
                        <Button
                          size="sm"
                          variant={roundedShares === Math.floor(result.shares) ? "default" : "outline"}
                          className="h-6 px-2 text-xs"
                          onClick={() => setRoundedShares(Math.floor(result.shares))}
                        >
                          Down ({Math.floor(result.shares)})
                        </Button>
                        <Button
                          size="sm"
                          variant={roundedShares === Math.ceil(result.shares) ? "default" : "outline"}
                          className="h-6 px-2 text-xs"
                          onClick={() => setRoundedShares(Math.ceil(result.shares))}
                        >
                          Up ({Math.ceil(result.shares)})
                        </Button>
                      </div>
                    </div>
                  ) : null}
                  {roundedShares !== null ? (
                    <ResultRow label="Shares to buy" value={fmt(finalShares, 0)} />
                  ) : null}
                  <ResultRow
                    label="Position value"
                    value={`$${fmt(finalValue)}`}
                  />
                  <ResultRow label="% of account" value={`${fmt(finalPct)}%`} />
                  <ResultRow
                    label="Stop loss distance"
                    value={`${fmt(result.stopLossPct)}%`}
                  />
                </div>
                <div className="flex justify-end gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleSave}
                    disabled={createHistory.isPending}
                  >
                    {createHistory.isPending ? "Saving..." : "Save to history"}
                  </Button>
                  <Button
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
          No calculations saved yet. Use "Save to history" in the Calculator tab.
        </span>
      </div>
    );
  }

  return (
    <div className="py-2">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Symbol</TableHead>
            <TableHead>Type</TableHead>
            <TableHead className="text-right">Entry</TableHead>
            <TableHead className="text-right">Stop</TableHead>
            <TableHead className="text-right">Shares</TableHead>
            <TableHead className="text-right">Value</TableHead>
            <TableHead className="text-right">% Acct</TableHead>
            <TableHead />
          </TableRow>
        </TableHeader>
        <TableBody>
          {history.data.map((entry) => (
            <TableRow key={entry.id}>
              <TableCell className="font-medium">{entry.symbol}</TableCell>
              <TableCell className="capitalize">{entry.positionType}</TableCell>
              <TableCell className="text-right tabular-nums">
                ${fmt(entry.entryPrice)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                ${fmt(entry.stopLoss)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {fmt(entry.shares)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                ${fmt(entry.positionValue)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {fmt(entry.accountPct)}%
              </TableCell>
              <TableCell>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-7 text-muted-foreground hover:text-destructive"
                  disabled={deleteEntry.isPending}
                  onClick={() => deleteEntry.mutate(entry.id)}
                >
                  <HugeiconsIcon
                    icon={Delete02Icon}
                    strokeWidth={2}
                    className="size-4"
                  />
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Rule tab
// ---------------------------------------------------------------------------

function RuleTab() {
  const ruleQuery = usePositionCalculatorRule();
  const upsertRule = useUpsertPositionCalculatorRule();

  const [accountBalance, setAccountBalance] = React.useState("");
  const [accountRisk, setAccountRisk] = React.useState("");
  const [maxStopLossPct, setMaxStopLossPct] = React.useState("");
  const [saved, setSaved] = React.useState(false);

  // Pre-fill form when existing rule loads
  React.useEffect(() => {
    if (ruleQuery.data) {
      setAccountBalance(ruleQuery.data.accountBalance.toString());
      setAccountRisk(ruleQuery.data.accountRisk.toString());
      setMaxStopLossPct(ruleQuery.data.maxStopLossPct.toString());
    }
  }, [ruleQuery.data]);

  async function handleSave() {
    const balance = parseFloat(accountBalance);
    const risk = parseFloat(accountRisk);
    const maxStop = parseFloat(maxStopLossPct);
    if (!isFinite(balance) || !isFinite(risk) || !isFinite(maxStop)) return;

    await upsertRule.mutateAsync({
      accountBalance: balance,
      accountRisk: risk,
      maxStopLossPct: maxStop,
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  return (
    <div className="grid gap-4 py-2">
      <div className="rounded-md border border-border bg-muted/40 p-3">
        <p className="text-sm font-medium">Auto-fill</p>
        <p className="mt-1 text-sm text-muted-foreground">
          Account balance and account risk will be pre-filled in the calculator
          each time you open it.
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
        <Field label="Default Account Balance ($)" htmlFor="rule-balance">
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

        <Field label="Default Account Risk (%)" htmlFor="rule-risk">
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

        <Field
          label="Max Stop Loss Distance (%)"
          htmlFor="rule-max-stop"
        >
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
        const riskAmount = isFinite(balance) && isFinite(risk) && balance > 0 && risk > 0
          ? balance * (risk / 100)
          : null;

        if (!riskAmount) return null;

        const lines = [
          `Account Balance: $${fmt(balance)}`,
          `Account Risk: ${fmt(risk)}%`,
          `Risk per Trade: $${fmt(riskAmount)}`,
        ];
        if (isFinite(maxStop) && maxStop > 0) {
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
  positionType: string;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  totalShares: number;
  positionValue: number;
};

function CreatePlanForm({
  seed,
  onDone,
}: {
  seed: PlanSeed;
  onDone: () => void;
}) {
  const createPlan = useCreatePositionCalculatorPlan();
  const [tranches, setTranches] = React.useState([
    { percent: "100", targetPrice: seed.entryPrice.toString() },
  ]);

  function addTranche() {
    setTranches((prev) => [...prev, { percent: "", targetPrice: seed.entryPrice.toString() }]);
  }

  function removeTranche(index: number) {
    setTranches((prev) => prev.filter((_, i) => i !== index));
  }

  function updateTranche(index: number, field: "percent" | "targetPrice", value: string) {
    setTranches((prev) =>
      prev.map((t, i) => (i === index ? { ...t, [field]: value } : t)),
    );
  }

  const totalPercent = tranches.reduce(
    (sum, t) => sum + (parseFloat(t.percent) || 0),
    0,
  );
  const isValid = totalPercent === 100 && tranches.every((t) => parseFloat(t.percent) > 0 && parseFloat(t.targetPrice) > 0);

  async function handleCreate() {
    if (!isValid) return;
    await createPlan.mutateAsync({
      ...seed,
      tranches: tranches.map((t) => {
        const pct = parseFloat(t.percent);
        return {
          percent: pct,
          shares: Math.round((pct / 100) * seed.totalShares * 100) / 100,
          targetPrice: parseFloat(t.targetPrice),
        };
      }),
    });
    onDone();
  }

  return (
    <div className="grid gap-3 py-2">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium">
          {seed.symbol} — {fmt(seed.totalShares)} shares
        </p>
        <Button size="sm" variant="ghost" className="h-6 px-2 text-xs" onClick={addTranche}>
          <HugeiconsIcon icon={PlusSignIcon} strokeWidth={2} className="mr-1 size-3" />
          Add tranche
        </Button>
      </div>

      {tranches.map((tranche, index) => {
        const pct = parseFloat(tranche.percent) || 0;
        const shares = Math.round((pct / 100) * seed.totalShares * 100) / 100;
        return (
          <div key={index} className="flex items-end gap-2">
            <Field label={`Tranche ${index + 1} (%)`}>
              <Input
                type="number"
                step="1"
                min="0"
                max="100"
                value={tranche.percent}
                onChange={(e) => updateTranche(index, "percent", e.target.value)}
                className="w-20"
              />
            </Field>
            <Field label="Target price">
              <Input
                type="number"
                step="0.01"
                min="0"
                value={tranche.targetPrice}
                onChange={(e) => updateTranche(index, "targetPrice", e.target.value)}
                className="w-28"
              />
            </Field>
            <span className="pb-2 text-xs tabular-nums text-muted-foreground">
              {fmt(shares)} shares
            </span>
            {tranches.length > 1 ? (
              <Button
                size="icon"
                variant="ghost"
                className="mb-1 size-7 text-muted-foreground hover:text-destructive"
                onClick={() => removeTranche(index)}
              >
                <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} className="size-3.5" />
              </Button>
            ) : null}
          </div>
        );
      })}

      {totalPercent !== 100 ? (
        <p className="text-xs text-destructive">
          Tranches must total 100% (currently {fmt(totalPercent, 0)}%)
        </p>
      ) : null}

      <div className="flex justify-end gap-2">
        <Button size="sm" variant="outline" onClick={onDone}>
          Cancel
        </Button>
        <Button
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

function PlanCard({ plan }: { plan: PositionCalculatorPlan }) {
  const updatePlan = useUpdatePositionCalculatorPlan();
  const deletePlan = useDeletePositionCalculatorPlan();
  const createHistory = useCreatePositionCalculatorHistory();
  const [editPrices, setEditPrices] = React.useState<Record<string, string>>(() => {
    const initial: Record<string, string> = {};
    for (const t of plan.tranches) {
      if (t.status === "planned") {
        initial[t.id] = t.targetPrice.toString();
      }
    }
    return initial;
  });

  const [completing, setCompleting] = React.useState(false);

  const filledCount = plan.tranches.filter((t) => t.status === "filled").length;

  function handlePriceBlur(trancheId: string) {
    const raw = editPrices[trancheId];
    const newPrice = parseFloat(raw);
    const tranche = plan.tranches.find((t) => t.id === trancheId);
    if (!tranche || !isFinite(newPrice) || newPrice <= 0 || newPrice === tranche.targetPrice) return;
    updatePlan.mutate({
      id: plan.id,
      input: { tranches: [{ id: trancheId, targetPrice: newPrice }] },
    });
  }

  async function handleTrancheStatus(trancheId: string, status: string) {
    // Build the next state of tranches after this update
    const nextTranches = plan.tranches.map((t) =>
      t.id === trancheId ? { ...t, status } : t,
    );
    const allResolved = nextTranches.every((t) => t.status !== "planned");
    const filledTranches = nextTranches.filter((t) => t.status === "filled");

    // If not all resolved yet, just update the tranche status
    if (!allResolved) {
      updatePlan.mutate({
        id: plan.id,
        input: { tranches: [{ id: trancheId, status }] },
      });
      return;
    }

    // All tranches resolved — determine outcome
    setCompleting(true);

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
        return;
      }

      // Use the latest edited prices for filled tranches
      const resolvedTranches = filledTranches.map((t) => {
        const editedPrice = editPrices[t.id];
        const price = editedPrice ? parseFloat(editedPrice) : t.targetPrice;
        return { ...t, targetPrice: isFinite(price) && price > 0 ? price : t.targetPrice };
      });

      // Calculate weighted average entry
      const totalFilledShares = resolvedTranches.reduce((sum, t) => sum + t.shares, 0);
      const weightedEntry =
        resolvedTranches.reduce((sum, t) => sum + t.shares * t.targetPrice, 0) / totalFilledShares;
      const positionValue = totalFilledShares * weightedEntry;
      const accountPct = (positionValue / plan.accountBalance) * 100;
      const stopLossPct = (Math.abs(weightedEntry - plan.stopLoss) / weightedEntry) * 100;

      // 1. Update the last tranche status
      await updatePlan.mutateAsync({
        id: plan.id,
        input: { tranches: [{ id: trancheId, status }] },
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
      });

      // 3. Mark plan as completed
      await updatePlan.mutateAsync({
        id: plan.id,
        input: { status: "completed" },
      });
    } finally {
      setCompleting(false);
    }
  }

  function handleCancel() {
    updatePlan.mutate({ id: plan.id, input: { status: "cancelled" } });
  }

  return (
    <div className="rounded-md border border-border p-3">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium">
            {plan.symbol}{" "}
            <span className="capitalize text-muted-foreground">{plan.positionType}</span>
          </p>
          <p className="text-xs text-muted-foreground">
            {fmt(plan.totalShares)} shares @ ${fmt(plan.entryPrice)} — {filledCount}/{plan.tranches.length} filled
          </p>
        </div>
        <div className="flex gap-1">
          {plan.status === "active" ? (
            <Button
              size="icon"
              variant="ghost"
              className="size-7 text-muted-foreground hover:text-yellow-600"
              onClick={handleCancel}
              disabled={updatePlan.isPending}
              title="Cancel plan"
            >
              <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} className="size-3.5" />
            </Button>
          ) : null}
          <Button
            size="icon"
            variant="ghost"
            className="size-7 text-muted-foreground hover:text-destructive"
            onClick={() => deletePlan.mutate(plan.id)}
            disabled={deletePlan.isPending}
            title="Delete plan"
          >
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} className="size-3.5" />
          </Button>
        </div>
      </div>

      {plan.status !== "active" ? (
        <p className="mt-2 text-xs font-medium capitalize text-muted-foreground">
          {plan.status}
        </p>
      ) : (
        <div className="mt-2 grid gap-1">
          {plan.tranches.map((tranche) => (
            <div
              key={tranche.id}
              className="flex items-center justify-between rounded bg-muted/40 px-2 py-1.5"
            >
              <div className="text-xs flex items-center gap-1">
                <span className="font-medium">{fmt(tranche.percent, 0)}%</span>
                <span className="text-muted-foreground">—</span>
                <span className="text-muted-foreground">{fmt(tranche.shares)} shares @</span>
                {tranche.status === "planned" ? (
                  <Input
                    type="number"
                    step="0.01"
                    min="0"
                    value={editPrices[tranche.id] ?? tranche.targetPrice.toString()}
                    onChange={(e) =>
                      setEditPrices((prev) => ({ ...prev, [tranche.id]: e.target.value }))
                    }
                    onBlur={() => handlePriceBlur(tranche.id)}
                    className="h-5 w-20 px-1 text-xs tabular-nums"
                  />
                ) : (
                  <span className="text-muted-foreground">${fmt(tranche.targetPrice)}</span>
                )}
              </div>
              <div className="flex gap-1">
                {tranche.status === "planned" ? (
                  <>
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-6 px-2 text-xs"
                      onClick={() => handleTrancheStatus(tranche.id, "filled")}
                      disabled={updatePlan.isPending || completing}
                    >
                      <HugeiconsIcon icon={CheckmarkCircle01Icon} strokeWidth={2} className="mr-1 size-3" />
                      Filled
                    </Button>
                    <Button
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
                  <span className={`text-xs font-medium capitalize ${tranche.status === "filled" ? "text-green-600 dark:text-green-400" : "text-muted-foreground"}`}>
                    {tranche.status}
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
      {completing ? (
        <p className="mt-2 text-xs text-muted-foreground">Completing...</p>
      ) : null}
    </div>
  );
}

function PlansTab({ seed, onClearSeed }: { seed: PlanSeed | null; onClearSeed: () => void }) {
  const plansQuery = usePositionCalculatorPlans();

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

  if (!plansQuery.data || plansQuery.data.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <span className="text-sm text-muted-foreground">
          No plans yet. Use "Plan this position" in the Calculator tab.
        </span>
      </div>
    );
  }

  return (
    <ScrollArea className="max-h-[400px] py-2">
      <div className="grid gap-3 pr-3">
        {plansQuery.data.map((plan) => (
          <PlanCard key={plan.id} plan={plan} />
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
  const ruleQuery = usePositionCalculatorRule();
  const [activeTab, setActiveTab] = React.useState("calculator");
  const [planSeed, setPlanSeed] = React.useState<PlanSeed | null>(null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg min-h-[480px]">
        <DialogHeader>
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

        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList>
            <TabsTrigger value="calculator">Calculator</TabsTrigger>
            <TabsTrigger value="plans">Plans</TabsTrigger>
            <TabsTrigger value="history">History</TabsTrigger>
            <TabsTrigger value="rule">Rule</TabsTrigger>
          </TabsList>

          <TabsContent value="calculator">
            <CalculatorTab
              rule={ruleQuery.data ?? null}
              onPlan={(data) => {
                setPlanSeed(data);
                setActiveTab("plans");
              }}
            />
          </TabsContent>

          <TabsContent value="plans">
            <PlansTab
              seed={planSeed}
              onClearSeed={() => setPlanSeed(null)}
            />
          </TabsContent>

          <TabsContent value="history">
            <HistoryTab />
          </TabsContent>

          <TabsContent value="rule">
            <RuleTab />
          </TabsContent>
        </Tabs>

        <div className="flex justify-end">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
