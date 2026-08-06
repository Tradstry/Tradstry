"use client";

import { InformationCircleIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type { ReactNode } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn, formatPnl } from "@/lib/utils";

const PERCENT_FMT = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});
const INT_FMT = new Intl.NumberFormat("en-US");

export function formatCurrency(value: number) {
  return formatPnl(value, { precision: "cents" });
}

export function formatPercent(value: number) {
  return `${PERCENT_FMT.format(value)}%`;
}

export function formatInt(value: number) {
  return INT_FMT.format(value);
}

export function formatR(value: number | null) {
  return value === null ? "—" : `${value.toFixed(2)}R`;
}

export function formatRatio(value: number | null) {
  return value === null ? "—" : `${value.toFixed(2)}x`;
}

export function formatNumber(value: number | null, digits = 2) {
  return value === null ? "—" : value.toFixed(digits);
}

// Seconds → compact human duration (e.g. "2h 15m", "8m", "45s").
export function formatDuration(secs: number | null) {
  if (secs === null || !Number.isFinite(secs) || secs <= 0) return "—";
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${s}s`;
}

export type MetricTone = "neutral" | "positive" | "negative";

export function toneForValue(value: number): MetricTone {
  if (value > 0) return "positive";
  if (value < 0) return "negative";
  return "neutral";
}

/** A stat card with an optional info tooltip. Requires a TooltipProvider ancestor. */
export function MetricCard({
  label,
  value,
  sublabel,
  info,
  tone = "neutral",
}: {
  label: string;
  value: string;
  sublabel?: string;
  info?: { what: string; formula?: string };
  tone?: MetricTone;
}) {
  return (
    <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
      <div className="flex items-center justify-between gap-2">
        <p className="text-[0.68rem] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
          {label}
        </p>
        {info && (
          <Tooltip>
            <TooltipTrigger
              aria-label={`What is ${label}?`}
              className="shrink-0 text-muted-foreground/50 transition-colors hover:text-foreground"
            >
              <HugeiconsIcon
                icon={InformationCircleIcon}
                className="size-3.5"
              />
            </TooltipTrigger>
            <TooltipContent
              side="top"
              align="end"
              className="flex-col items-start gap-1 text-left"
            >
              <span>{info.what}</span>
              {info.formula && (
                <span className="font-mono text-[0.7rem] text-background/70">
                  {info.formula}
                </span>
              )}
            </TooltipContent>
          </Tooltip>
        )}
      </div>
      <p
        className={cn(
          "mt-3 text-2xl font-semibold",
          tone === "positive" && "text-emerald-600",
          tone === "negative" && "text-rose-600",
        )}
      >
        {value}
      </p>
      {sublabel && (
        <p className="mt-1 text-xs text-muted-foreground">{sublabel}</p>
      )}
    </div>
  );
}

/** A titled section wrapper used across the analytics page. */
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
        <h2 className="text-sm font-semibold text-foreground">{title}</h2>
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}
