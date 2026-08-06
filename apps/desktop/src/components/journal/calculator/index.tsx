import { useCallback, useEffect, useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Calculator01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import {
  accounts,
  positionCalculatorRule,
  type PositionCalculatorRule,
} from "../../../backend";
import { HistoryPanel } from "./history-panel";
import { PlansPanel } from "./plans-panel";
import { RulePanel } from "./rule-panel";
import { SizingPanel } from "./sizing-panel";
import type { PlanSeed } from "./formulas";

export function CalculatorModal({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  // undefined = resolving, null = no account, string = active account id
  const [accountId, setAccountId] = useState<string | null | undefined>(
    undefined,
  );
  const [rule, setRule] = useState<PositionCalculatorRule | null>(null);
  const [ruleLoading, setRuleLoading] = useState(true);
  const [tab, setTab] = useState("calculator");
  const [planSeed, setPlanSeed] = useState<PlanSeed | null>(null);

  const reloadRule = useCallback((id: string) => {
    setRuleLoading(true);
    positionCalculatorRule(id)
      .then((r) => {
        setRule(r);
        setRuleLoading(false);
      })
      .catch(() => {
        setRule(null);
        setRuleLoading(false);
      });
  }, []);

  useEffect(() => {
    if (!open) return;
    accounts()
      .then((accs) => setAccountId(accs[0]?.id ?? null))
      .catch(() => setAccountId(null));
  }, [open]);

  useEffect(() => {
    if (accountId) reloadRule(accountId);
  }, [accountId, reloadRule]);

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

        {accountId === undefined ? (
          <p className="text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>
        ) : accountId === null ? (
          <p className="text-sm text-zinc-500 dark:text-zinc-400">
            No account yet — connect a brokerage to size positions.
          </p>
        ) : (
          <>
            <Tabs
              value={tab}
              onValueChange={setTab}
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
                <SizingPanel
                  rule={rule}
                  onPlan={(seed) => {
                    setPlanSeed(seed);
                    setTab("plans");
                  }}
                />
              </TabsContent>

              <TabsContent
                value="plans"
                className="min-h-0 overflow-y-auto pr-1"
              >
                <PlansPanel
                  seed={planSeed}
                  onClearSeed={() => setPlanSeed(null)}
                />
              </TabsContent>

              <TabsContent
                value="history"
                className="min-h-0 overflow-auto pr-1"
              >
                <HistoryPanel />
              </TabsContent>

              <TabsContent value="rule" className="min-h-0 overflow-y-auto pr-1">
                <RulePanel
                  accountId={accountId}
                  rule={rule}
                  loading={ruleLoading}
                  onSaved={() => reloadRule(accountId)}
                />
              </TabsContent>
            </Tabs>

            <div className="flex shrink-0 justify-end">
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                Close
              </Button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
