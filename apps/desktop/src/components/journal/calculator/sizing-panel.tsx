import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { NumberField } from "../../user-interface/number-field";
import { notify } from "../../user-interface/toast";
import {
  createPositionCalculatorHistory,
  type PositionCalculatorRule,
} from "../../../backend";
import {
  calculatePositionSize,
  fmt,
  getStopLossError,
  type PlanSeed,
  type PositionType,
} from "./formulas";

function ResultRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between py-1">
      <span className="text-sm text-zinc-500 dark:text-zinc-400">
        {label}
      </span>
      <span className="text-sm font-semibold tabular-nums text-zinc-900 dark:text-zinc-50">
        {value}
      </span>
    </div>
  );
}

export function SizingPanel({
  rule,
  onPlan,
}: {
  rule: PositionCalculatorRule | null;
  onPlan: (seed: PlanSeed) => void;
}) {
  const [symbol, setSymbol] = useState("");
  const [positionType, setPositionType] = useState<PositionType>("long");
  const [entryPrice, setEntryPrice] = useState("");
  const [stopLoss, setStopLoss] = useState("");
  const [accountBalance, setAccountBalance] = useState("");
  const [accountRisk, setAccountRisk] = useState("");
  const [roundedShares, setRoundedShares] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);

  // Prefill from the rule once, without clobbering anything already typed —
  // a rule that loads after the trader started typing a risk % must not wipe it.
  useEffect(() => {
    if (!rule) return;
    setAccountBalance((current) => current || String(rule.accountBalance));
    setAccountRisk((current) => current || String(rule.accountRisk));
  }, [rule]);

  function setNumeric(setter: (v: string) => void) {
    return (value: string) => {
      setter(value);
      setRoundedShares(null);
    };
  }

  const stopLossError = getStopLossError(
    parseFloat(entryPrice),
    parseFloat(stopLoss),
    positionType,
  );
  const result = calculatePositionSize({
    entryPrice: parseFloat(entryPrice),
    stopLoss: parseFloat(stopLoss),
    accountBalance: parseFloat(accountBalance),
    accountRisk: parseFloat(accountRisk),
    positionType,
  });
  const stopLossWarning =
    result && rule && result.stopLossPct > rule.maxStopLossPct
      ? `Stop loss distance (${fmt(result.stopLossPct)}%) exceeds your rule maximum of ${fmt(rule.maxStopLossPct)}%.`
      : null;

  async function handleSaveHistory() {
    if (!result) return;
    const entry = parseFloat(entryPrice);
    const balance = parseFloat(accountBalance);
    const finalShares = roundedShares ?? result.shares;
    const finalValue = finalShares * entry;
    const finalPct = (finalValue / balance) * 100;

    setSaving(true);
    try {
      await createPositionCalculatorHistory({
        symbol: symbol.trim() || "—",
        positionType,
        entryPrice: entry,
        stopLoss: parseFloat(stopLoss),
        accountBalance: balance,
        accountRisk: parseFloat(accountRisk),
        shares: finalShares,
        positionValue: finalValue,
        accountPct: finalPct,
        stopLossPct: result.stopLossPct,
      });
      notify.success("Saved to history.");
    } catch (e) {
      notify.error("Failed to save to history.", String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex flex-col gap-4 rounded-2xl border border-zinc-200/80 bg-white/85 p-5 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <div className="flex items-end justify-between gap-4">
        <div className="flex flex-col gap-2">
          <Label htmlFor="calc-symbol">Symbol</Label>
          <Input
            id="calc-symbol"
            value={symbol}
            onChange={(e) => {
              setSymbol(e.target.value);
              setRoundedShares(null);
            }}
            placeholder="AAPL"
            className="w-36"
          />
        </div>

        <div className="flex flex-col gap-2">
          <Label>Position type</Label>
          <div className="flex gap-1 rounded-lg border border-input bg-muted/40 p-1">
            <Button
              type="button"
              size="sm"
              variant={positionType === "long" ? "default" : "ghost"}
              className="flex-1"
              onClick={() => setPositionType("long")}
            >
              Long
            </Button>
            <Button
              type="button"
              size="sm"
              variant={positionType === "short" ? "default" : "ghost"}
              className="flex-1"
              onClick={() => setPositionType("short")}
            >
              Short
            </Button>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <NumberField
          label="Entry price"
          value={entryPrice}
          onChange={setNumeric(setEntryPrice)}
          placeholder="0.00"
          prefix="$"
        />
        <div className="flex flex-col gap-2">
          <NumberField
            label="Stop loss"
            value={stopLoss}
            onChange={setNumeric(setStopLoss)}
            placeholder="0.00"
            prefix="$"
          />
          {stopLossError ? (
            <p className="text-xs text-red-600 dark:text-red-400">
              {stopLossError}
            </p>
          ) : null}
        </div>
        <NumberField
          label="Account balance ($)"
          value={accountBalance}
          onChange={setNumeric(setAccountBalance)}
          placeholder="10000.00"
          prefix="$"
        />
        <NumberField
          label="Account risk (%)"
          value={accountRisk}
          onChange={setNumeric(setAccountRisk)}
          placeholder="1.00"
          suffix="%"
        />
      </div>

      {result
        ? (() => {
            const hasDecimals = result.shares % 1 !== 0;
            const finalShares = roundedShares ?? result.shares;
            const entry = parseFloat(entryPrice);
            const balance = parseFloat(accountBalance);
            const finalValue = finalShares * entry;
            const finalPct = (finalValue / balance) * 100;
            const stopDistance = Math.abs(
              parseFloat(entryPrice) - parseFloat(stopLoss),
            );
            const actualRisk = finalShares * stopDistance;
            const overBalance = finalValue > balance;
            const roundsToZero = Math.floor(result.shares) === 0;

            return (
              <>
                <Separator />
                {stopLossWarning ? (
                  <p className="rounded-md bg-yellow-500/10 px-3 py-2 text-xs text-yellow-600 dark:text-yellow-400">
                    {stopLossWarning}
                  </p>
                ) : null}
                {overBalance ? (
                  <p className="rounded-md bg-yellow-500/10 px-3 py-2 text-xs text-yellow-600 dark:text-yellow-400">
                    Position value (${fmt(finalValue)}) exceeds your account
                    balance (${fmt(balance)}).
                  </p>
                ) : null}
                {roundsToZero ? (
                  <p className="rounded-md bg-yellow-500/10 px-3 py-2 text-xs text-yellow-600 dark:text-yellow-400">
                    Rounding down gives 0 shares. Your risk budget is smaller
                    than one share's stop distance.
                  </p>
                ) : null}

                <div className="flex flex-col gap-1">
                  <ResultRow label="Shares (raw)" value={fmt(result.shares)} />
                  {hasDecimals ? (
                    <div className="flex items-center justify-between rounded-md bg-muted/60 px-3 py-2">
                      <span className="text-xs text-zinc-500 dark:text-zinc-400">
                        Round shares?
                      </span>
                      <div className="flex gap-1">
                        <Button
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
                    <span className="text-sm text-zinc-500 dark:text-zinc-400">
                      Risk
                    </span>
                    <span className="text-sm font-medium tabular-nums text-zinc-900 dark:text-zinc-50">
                      ${fmt(actualRisk)}
                      <span className="px-2 text-zinc-400 dark:text-zinc-600">
                        ·
                      </span>
                      {fmt(result.stopLossPct)}%
                    </span>
                  </div>
                  <ResultRow
                    label="Position value"
                    value={`$${fmt(finalValue)}`}
                  />
                  <div className="flex items-center justify-between py-0.5">
                    <span className="text-sm text-zinc-500 dark:text-zinc-400">
                      % of account
                    </span>
                    <span
                      className={`text-sm font-medium tabular-nums ${
                        overBalance
                          ? "text-red-600 dark:text-red-400"
                          : "text-zinc-900 dark:text-zinc-50"
                      }`}
                    >
                      {fmt(finalPct)}%
                    </span>
                  </div>
                </div>

                <div className="flex justify-end gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleSaveHistory}
                    disabled={saving}
                  >
                    {saving ? "Saving…" : "Save to history"}
                  </Button>
                  <Button
                    size="sm"
                    onClick={() =>
                      onPlan({
                        symbol: symbol.trim() || "—",
                        positionType,
                        entryPrice: entry,
                        stopLoss: parseFloat(stopLoss),
                        accountBalance: balance,
                        accountRisk: parseFloat(accountRisk),
                        totalShares: finalShares,
                        positionValue: finalShares * entry,
                      })
                    }
                  >
                    Plan this position
                  </Button>
                </div>
              </>
            );
          })()
        : null}
    </div>
  );
}
