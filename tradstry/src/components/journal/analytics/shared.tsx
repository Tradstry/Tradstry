import { type ReactNode } from "react";
import { InfoIcon } from "@phosphor-icons/react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export type Tone = "neutral" | "positive" | "negative";

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});
const intFmt = new Intl.NumberFormat("en-US");

/** Signed USD, zero-floored (e.g. "+$1,234.56" / "-$40.00" / "$0.00"). */
export function formatCurrency(v: number): string {
  if (v === 0) return usd.format(0);
  return `${v > 0 ? "+" : "-"}${usd.format(Math.abs(v))}`;
}
/** value is already 0..100, not a fraction. */
export function formatPercent(v: number): string {
  return `${v.toFixed(1)}%`;
}
export function formatInt(v: number): string {
  return intFmt.format(v);
}
export function formatR(v: number | null): string {
  return v == null ? "—" : `${v.toFixed(2)}R`;
}
export function formatRatio(v: number | null): string {
  return v == null ? "—" : `${v.toFixed(2)}x`;
}
export function formatNumber(v: number | null, digits = 2): string {
  return v == null ? "—" : v.toFixed(digits);
}
export function formatDuration(secs: number | null): string {
  if (secs == null || secs <= 0) return "—";
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${Math.floor(secs)}s`;
}

const toneClass: Record<Tone, string> = {
  neutral: "text-zinc-900 dark:text-zinc-50",
  positive: "text-emerald-600 dark:text-emerald-400",
  negative: "text-red-600 dark:text-red-400",
};

/** Crossfades the value when it changes so the range switch feels smooth. */
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

/** A single headline metric. Matches the dashboard MetricCard, plus an
 *  optional info tooltip explaining the metric. */
export function MetricCard({
  label,
  value,
  sublabel,
  tone = "neutral",
  info,
}: {
  label: string;
  value: string;
  sublabel: string;
  tone?: Tone;
  info?: string;
}) {
  return (
    <div className="rounded-2xl border border-zinc-200/80 bg-white/85 p-4 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <div className="flex items-center gap-1.5">
        <p className="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
          {label}
        </p>
        {info && (
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                className="text-zinc-400 transition-colors hover:text-zinc-600 dark:text-zinc-500 dark:hover:text-zinc-300"
                aria-label={`About ${label}`}
              >
                <InfoIcon size={12} weight="bold" />
              </button>
            </TooltipTrigger>
            <TooltipContent className="max-w-56 text-xs">
              {info}
            </TooltipContent>
          </Tooltip>
        )}
      </div>
      <div
        className={cn(
          "mt-2.5 text-2xl font-semibold tabular-nums",
          toneClass[tone],
        )}
      >
        <AnimatedValue value={value} />
      </div>
      <p className="mt-1 text-xs text-zinc-500 dark:text-zinc-400">{sublabel}</p>
    </div>
  );
}

/** A titled section with an optional description. */
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="space-y-3">
      <div>
        <h2 className="text-sm font-semibold text-zinc-900 dark:text-zinc-50">
          {title}
        </h2>
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}

/** A bordered panel used to frame charts. */
export function ChartPanel({
  title,
  children,
}: {
  title?: string;
  children: ReactNode;
}) {
  return (
    <div className="rounded-2xl border border-zinc-200/80 bg-white/85 p-4 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      {title && (
        <p className="mb-3 text-xs font-medium text-muted-foreground">
          {title}
        </p>
      )}
      {children}
    </div>
  );
}

/** Toned value for a P/L amount. */
export function toneForValue(v: number): Tone {
  return v > 0 ? "positive" : v < 0 ? "negative" : "neutral";
}
