import { useEffect, useState } from "react";
import { CaretDownIcon } from "@phosphor-icons/react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import {
  accounts,
  advancedAnalytics,
  type AdvancedAnalytics,
  type AnalyticsRange,
} from "../../../backend";
import { KpiCards } from "./kpi-cards";
import { EquityDrawdown } from "./equity-drawdown";
import { RMultiples } from "./r-multiples";
import { StreaksConsistency } from "./streaks-consistency";
import { Breakdowns } from "./breakdowns";
import { Behavioral } from "./behavioral";

const RANGE_OPTIONS: { value: AnalyticsRange; label: string }[] = [
  { value: "TODAY", label: "Today" },
  { value: "LAST_7_DAYS", label: "Last 7 days" },
  { value: "LAST_1_MONTH", label: "Last month" },
  { value: "LAST_3_MONTHS", label: "Last 3 months" },
  { value: "LAST_6_MONTHS", label: "Last 6 months" },
  { value: "YEAR_TO_DATE", label: "Year to date" },
  { value: "LAST_1_YEAR", label: "Last year" },
  { value: "ALL", label: "All time" },
];
const rangeLabel = (v: AnalyticsRange) =>
  RANGE_OPTIONS.find((o) => o.value === v)?.label ?? "All time";

function fmtDay(s: string): string {
  const d = new Date(`${s}T00:00:00`);
  return Number.isNaN(d.getTime())
    ? s
    : d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

function RangeSelect({
  value,
  onChange,
}: {
  value: AnalyticsRange;
  onChange: (v: AnalyticsRange) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex h-8 cursor-pointer items-center gap-1.5 rounded-lg border border-zinc-200 bg-white px-3 text-sm font-medium text-zinc-700 outline-none transition duration-150 hover:bg-zinc-50 focus-visible:outline-2 focus-visible:outline-blue-500 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-800"
        >
          {rangeLabel(value)}
          <CaretDownIcon size={13} className="text-zinc-400 dark:text-zinc-500" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuRadioGroup
          value={value}
          onValueChange={(v) => onChange(v as AnalyticsRange)}
        >
          {RANGE_OPTIONS.map((o) => (
            <DropdownMenuRadioItem key={o.value} value={o.value}>
              {o.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function AnalyticsSkeleton() {
  return (
    <div className="space-y-6">
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-28 rounded-2xl" />
        ))}
      </div>
      <Skeleton className="h-[320px] rounded-2xl" />
      <Skeleton className="h-[280px] rounded-2xl" />
    </div>
  );
}

export default function AnalyticsView() {
  const [accountId, setAccountId] = useState<string | null | undefined>(
    undefined,
  );
  const [range, setRange] = useState<AnalyticsRange>("LAST_1_MONTH");
  const [data, setData] = useState<AdvancedAnalytics | null>(null);
  const [fetching, setFetching] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    accounts()
      .then((a) => setAccountId(a[0]?.id ?? null))
      .catch(() => setAccountId(null));
  }, []);

  useEffect(() => {
    if (!accountId) return;
    let cancelled = false;
    setFetching(true);
    setError(null);
    advancedAnalytics(accountId, { range })
      .then((d) => {
        if (!cancelled) setData(d);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setFetching(false);
      });
    return () => {
      cancelled = true;
    };
  }, [accountId, range]);

  if (accountId === undefined) {
    return <p className="text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>;
  }
  if (accountId === null) {
    return (
      <p className="text-sm text-zinc-500 dark:text-zinc-400">
        No account yet — connect a brokerage to see your analytics.
      </p>
    );
  }

  const span =
    data?.rangeStart && data?.rangeEnd
      ? ` · ${fmtDay(data.rangeStart)} – ${fmtDay(data.rangeEnd)}`
      : "";

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          Deep performance metrics{span}
        </p>
        <RangeSelect value={range} onChange={setRange} />
      </div>

      {error ? (
        <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
          Couldn't load analytics: {error}
        </p>
      ) : !data ? (
        <AnalyticsSkeleton />
      ) : data.tradeCount === 0 ? (
        <div className="flex flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-zinc-300 p-12 text-center dark:border-zinc-700">
          <p className="text-sm font-medium text-zinc-700 dark:text-zinc-200">
            No trades in this range
          </p>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">
            Pick a wider range or log some trades to see your analytics.
          </p>
        </div>
      ) : (
        <div
          className={`flex flex-col gap-8 transition-opacity duration-200 ${fetching ? "opacity-60" : ""}`}
        >
          <KpiCards data={data} />
          <EquityDrawdown data={data} />
          <RMultiples data={data} />
          <StreaksConsistency data={data} />
          <Breakdowns data={data} />
          <Behavioral data={data} />
        </div>
      )}
    </div>
  );
}
