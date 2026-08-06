import { useEffect, useState } from "react";
import {
  advancedAnalytics,
  type AdvancedAnalytics,
  type AnalyticsRange,
} from "../../../backend";

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <p className="text-[0.65rem] font-semibold uppercase tracking-[0.14em] text-zinc-500 dark:text-zinc-400">
        {label}
      </p>
      <p className="text-sm font-semibold tabular-nums text-zinc-900 dark:text-zinc-50">
        {value}
      </p>
    </div>
  );
}

export default function DisciplineCard({
  accountId,
  range,
}: {
  accountId: string;
  range: AnalyticsRange;
}) {
  const [data, setData] = useState<AdvancedAnalytics | null>(null);
  const [fetching, setFetching] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
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

  if (!data && fetching) {
    return (
      <section className="rounded-2xl border border-zinc-200/80 bg-white/85 p-5 backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
        <div className="h-40 animate-pulse rounded-xl bg-zinc-100/70 dark:bg-zinc-900/50" />
      </section>
    );
  }

  if (!data) {
    return (
      <section className="rounded-2xl border border-zinc-200/80 bg-white/85 p-5 backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
        <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
          Couldn't load discipline metrics: {error}
        </p>
      </section>
    );
  }

  const { cleanVsFlawed, discipline: d } = data;
  const costIsBad = d.mistakeCost > 0;

  return (
    <section
      className={`rounded-2xl border border-zinc-200/80 bg-white/85 p-5 backdrop-blur-md transition-opacity duration-200 dark:border-zinc-800 dark:bg-zinc-900/70 ${fetching ? "opacity-60" : ""}`}
    >
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div>
          <p className="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
            Discipline
          </p>
          <div className="mt-2.5 flex flex-wrap items-baseline gap-x-2 gap-y-1 text-sm text-zinc-500 dark:text-zinc-400">
            <span className="font-semibold text-zinc-900 dark:text-zinc-50">
              {cleanVsFlawed.clean.tradeCount} clean
            </span>
            <span className="text-emerald-600 dark:text-emerald-400">
              {usd.format(cleanVsFlawed.clean.netProfit)}
            </span>
            <span>vs</span>
            <span className="font-semibold text-zinc-900 dark:text-zinc-50">
              {cleanVsFlawed.flawed.tradeCount} flawed
            </span>
            <span className="text-red-600 dark:text-red-400">
              {usd.format(cleanVsFlawed.flawed.netProfit)}
            </span>
          </div>
        </div>
        <div className="text-left md:text-right">
          <p className="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
            Mistake Cost
          </p>
          <p
            className={`mt-1 text-2xl font-semibold tabular-nums ${
              costIsBad
                ? "text-red-600 dark:text-red-400"
                : "text-zinc-900 dark:text-zinc-50"
            }`}
          >
            {usd.format(d.mistakeCost)}
          </p>
        </div>
      </div>

      <div className="mt-5 grid grid-cols-2 gap-4 border-t border-zinc-200/80 pt-4 sm:grid-cols-5 dark:border-zinc-800">
        <Stat
          label="Rule Adherence"
          value={
            d.avgRuleAdherence != null
              ? `${d.avgRuleAdherence.toFixed(1)}/5`
              : "—"
          }
        />
        <Stat
          label="Conviction"
          value={
            d.avgConviction != null ? `${d.avgConviction.toFixed(1)}/5` : "—"
          }
        />
        <Stat label="Revenge Trades" value={String(d.revengeTradeCount)} />
        <Stat label="30-min Breaks" value={String(d.broke30MinCount)} />
        <Stat
          label="Principle Violations"
          value={`${d.totalViolations} across ${d.tradesWithViolations} trades`}
        />
      </div>
    </section>
  );
}
