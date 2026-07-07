import { useEffect, useState } from "react";
import {
  accounts,
  journalAnalytics,
  type JournalAnalytics,
} from "../../backend";

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});

type Tone = "positive" | "negative" | "neutral";

function StatTile({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value: string;
  tone?: Tone;
}) {
  const valueColor =
    tone === "positive"
      ? "text-emerald-600 dark:text-emerald-400"
      : tone === "negative"
        ? "text-red-600 dark:text-red-400"
        : "text-zinc-900 dark:text-zinc-50";
  return (
    <div className="flex flex-col gap-1 rounded-lg border border-zinc-200/80 bg-white/85 px-4 py-3 backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <span className="text-xs font-medium uppercase tracking-wide text-zinc-500 dark:text-zinc-400">
        {label}
      </span>
      <span className={`text-xl font-semibold tabular-nums ${valueColor}`}>
        {value}
      </span>
    </div>
  );
}

export default function AnalyticsPanel() {
  const [data, setData] = useState<JournalAnalytics | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "empty" | "error">(
    "loading",
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const accs = await accounts();
        if (cancelled) return;
        if (accs.length === 0) {
          setState("empty");
          return;
        }
        const result = await journalAnalytics(accs[0].id, { range: "ALL" });
        if (cancelled) return;
        setData(result);
        setState("ready");
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
          setState("error");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (state === "loading") {
    return (
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {Array.from({ length: 4 }).map((_, i) => (
          <div
            key={i}
            className="h-[68px] animate-pulse rounded-lg border border-zinc-200/80 bg-zinc-100/70 dark:border-zinc-800 dark:bg-zinc-900/50"
          />
        ))}
      </div>
    );
  }

  if (state === "empty") {
    return (
      <p className="text-sm text-zinc-500 dark:text-zinc-400">
        No accounts yet — connect a brokerage to see analytics.
      </p>
    );
  }

  if (state === "error") {
    return (
      <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
        Couldn't load analytics: {error}
      </p>
    );
  }

  const d = data!;
  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      <StatTile label="Win Rate" value={`${d.winRate.toFixed(1)}%`} />
      <StatTile
        label="Net P&L"
        value={`${d.cumulativeProfit >= 0 ? "+" : ""}${usd.format(d.cumulativeProfit)}`}
        tone={d.cumulativeProfit >= 0 ? "positive" : "negative"}
      />
      <StatTile label="Profit Factor" value={d.profitFactor.toFixed(2)} />
      <StatTile label="Avg R:R" value={`${d.averageRiskToReward.toFixed(2)}R`} />
    </div>
  );
}
