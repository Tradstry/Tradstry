"use client";

import { AnalyticsUpIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { DashboardRangeSelect } from "@/components/dashboard/range-select";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useActiveWorkspace } from "@/components/workspaces";
import { useAdvancedAnalytics } from "@/hooks/analytics";
import { rangeSubtitleSuffix } from "@/lib/range-format";
import type { AnalyticsRange } from "@/lib/types/analytics";
import { cn } from "@/lib/utils";
import { Behavioral } from "./behavioral";
import { Breakdowns } from "./breakdowns";
import { PnlCalendar } from "./calendar";
import { EquityDrawdown } from "./equity-drawdown";
import { AnalyticsKpiCards } from "./kpi-cards";
import { RMultiples } from "./r-multiples";
import { StreaksConsistency } from "./streaks-consistency";

const TABS = [
  { value: "overview", label: "Overview" },
  { value: "risk", label: "Risk & R" },
  { value: "edges", label: "Edges" },
  { value: "behavior", label: "Behavior" },
] as const;

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
    <Empty className="border">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={AnalyticsUpIcon} strokeWidth={2} />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{body}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

export function Analytics() {
  const [range, setRange] = useState<AnalyticsRange>("LAST_1_MONTH");
  // Deep-linkable tab via ?tab= — read once on mount, sync with history (no
  // Next navigation, so no Suspense boundary needed for this client page).
  const [tab, setTab] = useState<string>(() => {
    if (typeof window === "undefined") return "overview";
    const t = new URLSearchParams(window.location.search).get("tab");
    return TABS.some((x) => x.value === t) ? (t as string) : "overview";
  });
  const onTabChange = (value: string) => {
    setTab(value);
    const url = new URL(window.location.href);
    url.searchParams.set("tab", value);
    window.history.replaceState(null, "", url);
  };
  const activeWorkspace = useActiveWorkspace();
  const { data, isLoading, isPending, isPlaceholderData, error } =
    useAdvancedAnalytics(activeWorkspace?.id ?? null, { range });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between gap-4">
        <div>
          <p className="text-sm text-muted-foreground">
            Deep performance metrics ·{" "}
            {rangeSubtitleSuffix(range, data?.rangeStart, data?.rangeEnd)}
          </p>
        </div>
        <DashboardRangeSelect value={range} onValueChange={setRange} />
      </div>

      {!activeWorkspace ? (
        <Notice
          title="No active workspace"
          body="Select a workspace to load analytics."
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
          <Tabs value={tab} onValueChange={onTabChange} className="gap-6">
            <TabsList>
              {TABS.map((t) => (
                <TabsTrigger key={t.value} value={t.value}>
                  {t.label}
                </TabsTrigger>
              ))}
            </TabsList>

            <div
              className={cn(
                "transition-opacity duration-200",
                isPlaceholderData && "opacity-60",
              )}
            >
              <TabsContent value="overview" className="flex flex-col gap-8">
                <AnalyticsKpiCards data={data} range={range} />
                <EquityDrawdown data={data} />
              </TabsContent>
              <TabsContent value="risk" className="flex flex-col gap-8">
                <RMultiples data={data} />
                <StreaksConsistency data={data} />
              </TabsContent>
              <TabsContent value="edges" className="flex flex-col gap-8">
                <Breakdowns data={data} />
                <PnlCalendar />
              </TabsContent>
              <TabsContent value="behavior" className="flex flex-col gap-8">
                <Behavioral data={data} />
              </TabsContent>
            </div>
          </Tabs>
        </TooltipProvider>
      )}
    </div>
  );
}
