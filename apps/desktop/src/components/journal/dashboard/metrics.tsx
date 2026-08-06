import { useEffect, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  journalAnalytics,
  type AnalyticsRange,
  type JournalAnalytics,
} from "../../../backend";

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});
const pct = (v: number) => `${v.toFixed(2)}%`;
const signedUsd = (v: number) => `${v >= 0 ? "+" : ""}${usd.format(v)}`;

type Tone = "neutral" | "positive" | "negative";

/**
 * Crossfades between values: when `value` changes, the old one fades + slides
 * up while the new one fades in from below (they overlap in one grid cell so
 * layout never shifts). Falls back to a plain fade under reduced motion.
 */
function AnimatedValue({ value }: { value: string }) {
  const reduce = useReducedMotion();
  return (
    <span className="grid">
      <AnimatePresence initial={false}>
        <motion.span
          key={value}
          initial={reduce ? { opacity: 0 } : { opacity: 0, y: "0.35em" }}
          animate={{ opacity: 1, y: 0 }}
          exit={reduce ? { opacity: 0 } : { opacity: 0, y: "-0.35em" }}
          transition={{ duration: 0.22, ease: "easeOut" }}
          className="col-start-1 row-start-1"
        >
          {value}
        </motion.span>
      </AnimatePresence>
    </span>
  );
}

function MetricCard({
  label,
  value,
  sublabel,
  tone = "neutral",
}: {
  label: string;
  value: string;
  sublabel: string;
  tone?: Tone;
}) {
  const color =
    tone === "positive"
      ? "text-emerald-600 dark:text-emerald-400"
      : tone === "negative"
        ? "text-red-600 dark:text-red-400"
        : "text-zinc-900 dark:text-zinc-50";
  return (
    <div className="rounded-xl border border-zinc-200/80 bg-white/85 p-4 backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <p className="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
        {label}
      </p>
      <div className={`mt-2.5 text-2xl font-semibold tabular-nums ${color}`}>
        <AnimatedValue value={value} />
      </div>
      <p className="mt-1 text-xs text-zinc-500 dark:text-zinc-400">{sublabel}</p>
    </div>
  );
}

export default function MetricsGrid({
  accountId,
  range,
}: {
  accountId: string;
  range: AnalyticsRange;
}) {
  const [data, setData] = useState<JournalAnalytics | null>(null);
  const [fetching, setFetching] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setFetching(true);
    setError(null);
    journalAnalytics(accountId, { range })
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

  // First load only: show skeletons. Later range changes keep the old values
  // visible (dimmed) so switching never flashes empty cards.
  if (!data && fetching) {
    return (
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {Array.from({ length: 8 }).map((_, i) => (
          <div
            key={i}
            className="h-28 animate-pulse rounded-xl border border-zinc-200/80 bg-zinc-100/70 dark:border-zinc-800 dark:bg-zinc-900/50"
          />
        ))}
      </div>
    );
  }

  if (!data) {
    return (
      <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
        Couldn't load metrics: {error}
      </p>
    );
  }

  const d = data;
  return (
    <div
      className={`grid gap-4 transition-opacity duration-200 sm:grid-cols-2 xl:grid-cols-4 ${fetching ? "opacity-60" : ""}`}
    >
      <MetricCard
        label="Cumulative Profit"
        value={signedUsd(d.cumulativeProfit)}
        sublabel="Net realized P/L"
        tone={d.cumulativeProfit >= 0 ? "positive" : "negative"}
      />
      <MetricCard
        label="Win Rate"
        value={pct(d.winRate)}
        sublabel="Winners ÷ closed trades"
      />
      <MetricCard
        label="Average R:R"
        value={`${d.averageRiskToReward.toFixed(2)}R`}
        sublabel="Avg realized reward-to-risk"
      />
      <MetricCard
        label="Profit Factor"
        value={d.profitFactor != null ? `${d.profitFactor.toFixed(2)}x` : "—"}
        sublabel="Gross profit ÷ gross loss"
      />
      <MetricCard
        label="Average Gain"
        value={usd.format(d.averageGain)}
        sublabel={`+${pct(d.averageGainPct)} avg on winners`}
        tone="positive"
      />
      <MetricCard
        label="Average Loss"
        value={usd.format(-d.averageLoss)}
        sublabel={`-${pct(d.averageLossPct)} avg on losers`}
        tone="negative"
      />
      <MetricCard
        label="Biggest Win"
        value={d.biggestWin?.symbol ?? "—"}
        sublabel={d.biggestWin ? usd.format(d.biggestWin.amount) : "No winning trade"}
        tone={d.biggestWin ? "positive" : "neutral"}
      />
      <MetricCard
        label="Biggest Loss"
        value={d.biggestLoss?.symbol ?? "—"}
        sublabel={
          d.biggestLoss ? usd.format(d.biggestLoss.amount) : "No losing trade"
        }
        tone={d.biggestLoss ? "negative" : "neutral"}
      />
    </div>
  );
}
