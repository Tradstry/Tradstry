import { useCallback, useEffect, useState } from "react";
import {
  CheckCircleIcon,
  PlusIcon,
  TrashIcon,
  XCircleIcon,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { notify } from "../../user-interface/toast";
import {
  createPositionCalculatorHistory,
  createPositionCalculatorPlan,
  deletePositionCalculatorPlan,
  positionCalculatorPlans,
  updatePositionCalculatorPlan,
  type PositionCalculatorPlan,
  type Tranche,
} from "../../../backend";
import { fmt, type PlanSeed } from "./formulas";

type TrancheDraft = {
  id: string;
  percent: string;
  targetPrice: string;
};

function createTrancheDraft(targetPrice: number, percent = ""): TrancheDraft {
  return {
    id: crypto.randomUUID(),
    percent,
    targetPrice: targetPrice.toString(),
  };
}

// The last tranche absorbs whatever the others leave, so the total is 100%
// without hand-balancing. Editing the last one directly is still respected.
function rebalanceLast(list: TrancheDraft[]): TrancheDraft[] {
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

function CreatePlanForm({
  seed,
  onDone,
}: {
  seed: PlanSeed;
  onDone: () => void;
}) {
  const [tranches, setTranches] = useState<TrancheDraft[]>([
    createTrancheDraft(seed.entryPrice, "100"),
  ]);
  const [creating, setCreating] = useState(false);

  function addTranche() {
    setTranches((prev) =>
      rebalanceLast([...prev, createTrancheDraft(seed.entryPrice)]),
    );
  }

  function removeTranche(trancheId: string) {
    setTranches((prev) =>
      rebalanceLast(prev.filter((t) => t.id !== trancheId)),
    );
  }

  function updateTranche(
    trancheId: string,
    field: "percent" | "targetPrice",
    value: string,
  ) {
    setTranches((prev) => {
      const next = prev.map((t) =>
        t.id === trancheId ? { ...t, [field]: value } : t,
      );
      const isLast = prev[prev.length - 1]?.id === trancheId;
      return field === "percent" && !isLast ? rebalanceLast(next) : next;
    });
  }

  const totalPercent = tranches.reduce(
    (sum, t) => sum + (parseFloat(t.percent) || 0),
    0,
  );
  const isValid =
    totalPercent === 100 &&
    tranches.every(
      (t) => parseFloat(t.percent) > 0 && parseFloat(t.targetPrice) > 0,
    );

  async function handleCreate() {
    if (!isValid || creating) return;
    setCreating(true);
    try {
      const fullTranches: Tranche[] = tranches.map((t) => {
        const pct = parseFloat(t.percent);
        return {
          id: crypto.randomUUID(),
          percent: pct,
          shares: Math.round((pct / 100) * seed.totalShares * 100) / 100,
          targetPrice: parseFloat(t.targetPrice),
          status: "planned",
          filledAt: null,
        };
      });
      await createPositionCalculatorPlan({
        symbol: seed.symbol,
        positionType: seed.positionType,
        entryPrice: seed.entryPrice,
        stopLoss: seed.stopLoss,
        accountBalance: seed.accountBalance,
        accountRisk: seed.accountRisk,
        totalShares: seed.totalShares,
        positionValue: seed.positionValue,
        tranchesJson: JSON.stringify(fullTranches),
      });
      notify.success(`${seed.symbol} plan created.`);
      onDone();
    } catch (e) {
      notify.error("Failed to create plan.", String(e));
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="flex flex-col gap-3 rounded-2xl border border-zinc-200/80 bg-white/85 p-5 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-zinc-900 dark:text-zinc-50">
          {seed.symbol} — {fmt(seed.totalShares)} shares
        </p>
        <Button
          size="sm"
          variant="ghost"
          className="h-6 px-2 text-xs"
          onClick={addTranche}
        >
          <PlusIcon size={13} weight="bold" />
          Add tranche
        </Button>
      </div>

      {tranches.map((tranche, index) => {
        const pct = parseFloat(tranche.percent) || 0;
        const shares = Math.round((pct / 100) * seed.totalShares * 100) / 100;
        const target = parseFloat(tranche.targetPrice);
        // What this tranche loses if it fills at its target and the stop
        // hits. Signed by position type: a target on the wrong side of the
        // stop isn't a risk figure, so nothing is shown rather than abs().
        const riskPerShare =
          seed.positionType === "short"
            ? seed.stopLoss - target
            : target - seed.stopLoss;
        const trancheRisk =
          Number.isFinite(riskPerShare) && riskPerShare > 0 && shares > 0
            ? shares * riskPerShare
            : null;
        return (
          <div key={tranche.id} className="flex items-end gap-2">
            <div className="flex flex-col gap-2">
              <Label>Tranche {index + 1} (%)</Label>
              <Input
                type="number"
                step="1"
                min="0"
                value={tranche.percent}
                onChange={(e) =>
                  updateTranche(tranche.id, "percent", e.target.value)
                }
                className="w-20"
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label>Target price</Label>
              <Input
                type="number"
                step="0.01"
                min="0"
                value={tranche.targetPrice}
                onChange={(e) =>
                  updateTranche(tranche.id, "targetPrice", e.target.value)
                }
                className="w-28"
              />
            </div>
            <span className="pb-1.5 text-xs tabular-nums text-zinc-500 dark:text-zinc-400">
              {fmt(shares)} shares
              {trancheRisk != null ? (
                <>
                  <span className="px-1.5">·</span>${fmt(trancheRisk)} risk
                </>
              ) : null}
            </span>
            {tranches.length > 1 ? (
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label="Remove tranche"
                className="mb-0.5 text-muted-foreground hover:text-destructive"
                onClick={() => removeTranche(tranche.id)}
              >
                <TrashIcon size={14} />
              </Button>
            ) : null}
          </div>
        );
      })}

      {totalPercent !== 100 ? (
        <p className="text-xs text-red-600 dark:text-red-400">
          Tranches must total 100% (currently {fmt(totalPercent, 0)}%)
        </p>
      ) : null}

      <div className="flex justify-end gap-2 pt-1">
        <Button size="sm" variant="outline" onClick={onDone}>
          Cancel
        </Button>
        <Button size="sm" onClick={handleCreate} disabled={!isValid || creating}>
          {creating ? "Creating…" : "Create plan"}
        </Button>
      </div>
    </div>
  );
}

function PlanCard({
  plan,
  onChanged,
}: {
  plan: PositionCalculatorPlan;
  onChanged: () => void;
}) {
  const [editPrices, setEditPrices] = useState<Record<string, string>>(() => {
    const initial: Record<string, string> = {};
    for (const t of plan.tranches) {
      if (t.status === "planned") initial[t.id] = t.targetPrice.toString();
    }
    return initial;
  });
  const [busy, setBusy] = useState(false);

  const filledCount = plan.tranches.filter(
    (t) => t.status === "filled",
  ).length;

  // Backend replaces the whole tranches array on write (no per-field patch),
  // so every tranche edit re-sends the full, locally-updated array.
  async function persistTranches(next: Tranche[]) {
    setBusy(true);
    try {
      await updatePositionCalculatorPlan(plan.id, {
        tranchesJson: JSON.stringify(next),
      });
      onChanged();
    } catch (e) {
      notify.error("Failed to update plan.", String(e));
    } finally {
      setBusy(false);
    }
  }

  function handlePriceBlur(trancheId: string) {
    const raw = editPrices[trancheId];
    const newPrice = parseFloat(raw);
    const tranche = plan.tranches.find((t) => t.id === trancheId);
    if (
      !tranche ||
      !Number.isFinite(newPrice) ||
      newPrice <= 0 ||
      newPrice === tranche.targetPrice
    )
      return;
    const next = plan.tranches.map((t) =>
      t.id === trancheId ? { ...t, targetPrice: newPrice } : t,
    );
    persistTranches(next);
  }

  // Mirrors the web calculator: when the last tranche resolves, roll the plan up
  // into a History entry at the shares-weighted average fill price and mark it
  // completed (or cancel if nothing filled). All local/offline.
  async function handleTrancheStatus(trancheId: string, status: string) {
    const next: Tranche[] = plan.tranches.map((t) =>
      t.id === trancheId
        ? {
            ...t,
            status,
            filledAt: status === "filled" ? new Date().toISOString() : null,
          }
        : t,
    );
    const allResolved = next.every((t) => t.status !== "planned");
    const filled = next.filter((t) => t.status === "filled");

    if (!allResolved) {
      persistTranches(next);
      return;
    }

    setBusy(true);
    try {
      if (filled.length === 0) {
        await updatePositionCalculatorPlan(plan.id, {
          tranchesJson: JSON.stringify(next),
          status: "cancelled",
        });
        notify.success(`No tranches filled — ${plan.symbol} plan cancelled.`);
        onChanged();
        return;
      }

      // Weighted-average entry over filled tranches, honoring edited prices.
      const resolved = filled.map((t) => {
        const edited = editPrices[t.id];
        const price = edited ? parseFloat(edited) : t.targetPrice;
        return {
          ...t,
          targetPrice: Number.isFinite(price) && price > 0 ? price : t.targetPrice,
        };
      });
      const totalShares = resolved.reduce((s, t) => s + t.shares, 0);
      const weightedEntry =
        resolved.reduce((s, t) => s + t.shares * t.targetPrice, 0) / totalShares;
      const positionValue = totalShares * weightedEntry;
      const accountPct = (positionValue / plan.accountBalance) * 100;
      const stopLossPct =
        (Math.abs(weightedEntry - plan.stopLoss) / weightedEntry) * 100;

      await updatePositionCalculatorPlan(plan.id, {
        tranchesJson: JSON.stringify(next),
      });
      await createPositionCalculatorHistory({
        symbol: plan.symbol,
        positionType: plan.positionType,
        entryPrice: weightedEntry,
        stopLoss: plan.stopLoss,
        accountBalance: plan.accountBalance,
        accountRisk: plan.accountRisk,
        shares: totalShares,
        positionValue,
        accountPct,
        stopLossPct,
      });
      await updatePositionCalculatorPlan(plan.id, { status: "completed" });
      notify.success(
        `${plan.symbol} plan completed — ${fmt(totalShares, 0)} shares @ $${fmt(weightedEntry)}. Moved to History.`,
      );
      onChanged();
    } catch (e) {
      notify.error("Failed to resolve plan.", String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleStatus(status: string) {
    setBusy(true);
    try {
      await updatePositionCalculatorPlan(plan.id, { status });
      notify.success(`${plan.symbol} plan marked ${status}.`);
      onChanged();
    } catch (e) {
      notify.error("Failed to update plan.", String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete() {
    setBusy(true);
    try {
      await deletePositionCalculatorPlan(plan.id);
      notify.success(`${plan.symbol} plan deleted.`);
      onChanged();
    } catch (e) {
      notify.error("Failed to delete plan.", String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="rounded-2xl border border-zinc-200/80 bg-white/85 p-4 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-zinc-900 dark:text-zinc-50">
            {plan.symbol}{" "}
            <span className="capitalize text-zinc-500 dark:text-zinc-400">
              {plan.positionType}
            </span>
          </p>
          <p className="text-xs text-zinc-500 dark:text-zinc-400">
            {fmt(plan.totalShares)} shares @ ${fmt(plan.entryPrice)} —{" "}
            {filledCount}/{plan.tranches.length} filled
          </p>
        </div>
        <div className="flex gap-1">
          {plan.status === "active" ? (
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label="Cancel plan"
              className="text-muted-foreground hover:text-amber-600"
              onClick={() => handleStatus("cancelled")}
              disabled={busy}
            >
              <XCircleIcon size={15} />
            </Button>
          ) : null}
          {plan.status === "active" ? (
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label="Mark completed"
              className="text-muted-foreground hover:text-emerald-600"
              onClick={() => handleStatus("completed")}
              disabled={busy}
            >
              <CheckCircleIcon size={15} />
            </Button>
          ) : null}
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="Delete plan"
            className="text-muted-foreground hover:text-destructive"
            onClick={handleDelete}
            disabled={busy}
          >
            <TrashIcon size={14} />
          </Button>
        </div>
      </div>

      {plan.status !== "active" ? (
        <p className="mt-2 text-xs font-medium capitalize text-zinc-500 dark:text-zinc-400">
          {plan.status}
        </p>
      ) : (
        <div className="mt-2 flex flex-col gap-1">
          {plan.tranches.map((tranche) => (
            <div
              key={tranche.id}
              className="flex items-center justify-between rounded bg-muted/40 px-2 py-1.5"
            >
              <div className="flex items-center gap-1 text-xs">
                <span className="font-medium text-zinc-900 dark:text-zinc-50">
                  {fmt(tranche.percent, 0)}%
                </span>
                <span className="text-zinc-400 dark:text-zinc-600">—</span>
                <span className="text-zinc-500 dark:text-zinc-400">
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
                    className="h-5 w-20 px-1 text-xs tabular-nums"
                  />
                ) : (
                  <span className="text-zinc-500 dark:text-zinc-400">
                    ${fmt(tranche.targetPrice)}
                  </span>
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
                      disabled={busy}
                    >
                      Filled
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-6 px-2 text-xs text-zinc-500 dark:text-zinc-400"
                      onClick={() =>
                        handleTrancheStatus(tranche.id, "skipped")
                      }
                      disabled={busy}
                    >
                      Skip
                    </Button>
                  </>
                ) : (
                  <span
                    className={`text-xs font-medium capitalize ${
                      tranche.status === "filled"
                        ? "text-emerald-600 dark:text-emerald-400"
                        : "text-zinc-500 dark:text-zinc-400"
                    }`}
                  >
                    {tranche.status}
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function PlansPanel({
  seed,
  onClearSeed,
}: {
  seed: PlanSeed | null;
  onClearSeed: () => void;
}) {
  const [plans, setPlans] = useState<PositionCalculatorPlan[] | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    setState("loading");
    positionCalculatorPlans()
      .then((p) => {
        setPlans(p);
        setState("ready");
      })
      .catch((e) => {
        setError(String(e));
        setState("error");
      });
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  if (seed) {
    return (
      <CreatePlanForm
        seed={seed}
        onDone={() => {
          onClearSeed();
          reload();
        }}
      />
    );
  }

  if (state === "loading" && !plans) {
    return (
      <p className="text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>
    );
  }

  if (state === "error") {
    return (
      <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
        Couldn't load plans: {error}
      </p>
    );
  }

  if (!plans || plans.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-zinc-300 p-12 text-center dark:border-zinc-700">
        <p className="text-sm font-medium text-zinc-700 dark:text-zinc-200">
          No plans yet
        </p>
        <p className="text-sm text-zinc-500 dark:text-zinc-400">
          Use "Plan this position" in the Calculator tab to lay out tranches
          for a trade.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {plans.map((plan) => (
        <PlanCard key={plan.id} plan={plan} onChanged={reload} />
      ))}
    </div>
  );
}
