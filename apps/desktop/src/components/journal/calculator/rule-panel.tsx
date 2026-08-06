import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { NumberField } from "../../user-interface/number-field";
import { notify } from "../../user-interface/toast";
import {
  upsertPositionCalculatorRule,
  type PositionCalculatorRule,
} from "../../../backend";
import { fmt } from "./formulas";

export function RulePanel({
  accountId,
  rule,
  loading,
  onSaved,
}: {
  accountId: string;
  rule: PositionCalculatorRule | null;
  loading: boolean;
  onSaved: () => void;
}) {
  const [accountBalance, setAccountBalance] = useState("");
  const [accountRisk, setAccountRisk] = useState("");
  const [maxStopLossPct, setMaxStopLossPct] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!rule) return;
    setAccountBalance(String(rule.accountBalance));
    setAccountRisk(String(rule.accountRisk));
    setMaxStopLossPct(String(rule.maxStopLossPct));
  }, [rule]);

  const balance = parseFloat(accountBalance);
  const risk = parseFloat(accountRisk);
  const riskAmount =
    Number.isFinite(balance) &&
    Number.isFinite(risk) &&
    balance > 0 &&
    risk > 0
      ? balance * (risk / 100)
      : null;

  async function handleSave() {
    const maxStop = parseFloat(maxStopLossPct);
    if (
      !Number.isFinite(balance) ||
      !Number.isFinite(risk) ||
      !Number.isFinite(maxStop)
    ) {
      notify.error("Enter a balance, risk %, and max stop-loss % first.");
      return;
    }
    setSaving(true);
    try {
      await upsertPositionCalculatorRule({
        accountId,
        accountBalance: balance,
        accountRisk: risk,
        maxStopLossPct: maxStop,
      });
      notify.success("Rule saved.");
      onSaved();
    } catch (e) {
      notify.error("Failed to save rule.", String(e));
    } finally {
      setSaving(false);
    }
  }

  if (loading && !rule) {
    return (
      <p className="text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>
    );
  }

  return (
    <div className="flex flex-col gap-4 rounded-2xl border border-zinc-200/80 bg-white/85 p-5 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <div>
        <h3 className="text-[15px] font-semibold text-zinc-900 dark:text-zinc-50">
          Position-sizing rule
        </h3>
        <p className="mt-1 text-xs text-zinc-500 dark:text-zinc-400">
          Sets the defaults the calculator starts from, and the max stop-loss
          distance it warns you about.
        </p>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <NumberField
          label="Account balance ($)"
          value={accountBalance}
          onChange={setAccountBalance}
          placeholder="10000.00"
          prefix="$"
        />
        <NumberField
          label="Account risk (%)"
          value={accountRisk}
          onChange={setAccountRisk}
          placeholder="1.00"
          suffix="%"
        />
        <div className="flex flex-col gap-2">
          <Label>Risk per trade</Label>
          <div className="flex h-9 items-center rounded-lg border border-input bg-muted/30 px-2.5 text-sm tabular-nums text-muted-foreground">
            {riskAmount != null ? `$${fmt(riskAmount)}` : "—"}
          </div>
        </div>
        <NumberField
          label="Max stop-loss distance (%)"
          value={maxStopLossPct}
          onChange={setMaxStopLossPct}
          placeholder="2.00"
          suffix="%"
        />
      </div>

      <div className="flex justify-end">
        <Button size="sm" onClick={handleSave} disabled={saving}>
          {saving ? "Saving…" : "Save rule"}
        </Button>
      </div>
    </div>
  );
}
