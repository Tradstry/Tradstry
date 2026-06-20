"use client";

import { useState } from "react";
import { useActiveAccount } from "@/components/accounts";
import { DashboardRangeSelect } from "@/components/dashboard/range-select";
import { Skeleton } from "@/components/ui/skeleton";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useAdvancedAnalytics } from "@/hooks/analytics";
import { rangeSubtitleSuffix } from "@/lib/range-format";
import type { AnalyticsRange } from "@/lib/types/analytics";
import { cn } from "@/lib/utils";
import { Behavioral } from "./behavioral";
import { Breakdowns } from "./breakdowns";
import { EquityDrawdown } from "./equity-drawdown";
import { AnalyticsKpiCards } from "./kpi-cards";
import { RMultiples } from "./r-multiples";
import { StreaksConsistency } from "./streaks-consistency";

function LoadingState() {
  return (
    <div className="space-y-6">
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {["a", "b", "c", "d", "e", "f"].map((key) => (
          <Skeleton key={key} className="h-32 rounded-2xl" />
        ))}
      </div>
      <Skeleton className="h-[300px] rounded-2xl" />
      <Skeleton className="h-[260px] rounded-2xl" />
    </div>
  );
}

function Notice({ title, body }: { title: string; body: string }) {
  return (
    <section className="rounded-2xl border bg-background/80 p-6">
      <p className="text-sm font-medium text-foreground">{title}</p>
      <p className="mt-2 text-sm text-muted-foreground">{body}</p>
    </section>
  );
}

export function Analytics() {
  const [range, setRange] = useState<AnalyticsRange>("LAST_1_MONTH");
  const activeAccount = useActiveAccount();
  const { data, isLoading, isPending, isPlaceholderData, error } =
    useAdvancedAnalytics(activeAccount?.id ?? null, { range });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between gap-4">
        <div>
          <h1 className="text-lg font-semibold text-foreground">Analytics</h1>
          <p className="text-sm text-muted-foreground">
            Deep performance metrics ·{" "}
            {rangeSubtitleSuffix(range, data?.rangeStart, data?.rangeEnd)}
          </p>
        </div>
        <DashboardRangeSelect value={range} onValueChange={setRange} />
      </div>

      {!activeAccount ? (
        <Notice
          title="No active account"
          body="Select an account to load analytics."
        />
      ) : isLoading || isPending ? (
        <LoadingState />
      ) : error instanceof Error ? (
        <section className="rounded-2xl border border-rose-200 bg-rose-50 p-6 text-rose-700">
          <p className="text-sm font-semibold uppercase tracking-[0.2em]">
            Analytics Error
          </p>
          <p className="mt-2 text-sm">{error.message}</p>
        </section>
      ) : !data ? null : data.tradeCount === 0 ? (
        <Notice
          title="No trades in this range"
          body="Pick a wider date range, or log some trades to see your metrics."
        />
      ) : (
        <TooltipProvider delayDuration={150}>
          <div
            className={cn(
              "flex flex-col gap-8 transition-opacity duration-200",
              isPlaceholderData && "opacity-60",
            )}
          >
            <AnalyticsKpiCards data={data} range={range} />
            <EquityDrawdown data={data} />
            <RMultiples data={data} />
            <StreaksConsistency data={data} />
            <Breakdowns data={data} />
            <Behavioral data={data} />
          </div>
        </TooltipProvider>
      )}
    </div>
  );
}
