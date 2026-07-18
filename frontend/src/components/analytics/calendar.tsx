"use client";

import { ArrowLeft01Icon, ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { useActiveAccount } from "@/components/accounts";
import { Button } from "@/components/ui/button";
import { useCalendarAnalytics } from "@/hooks/analytics";
import { cn } from "@/lib/utils";
import { formatCurrency, formatInt, Section } from "./shared";

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

export function PnlCalendar() {
  const account = useActiveAccount();
  const now = new Date();
  const [cursor, setCursor] = useState({
    year: now.getFullYear(),
    month: now.getMonth() + 1,
  });

  const { data, isPlaceholderData } = useCalendarAnalytics(
    account?.id ?? null,
    cursor.year,
    cursor.month,
  );

  // Normalize month overflow/underflow through Date so Dec→Jan rolls the year.
  const step = (delta: number) =>
    setCursor((c) => {
      const d = new Date(c.year, c.month - 1 + delta, 1);
      return { year: d.getFullYear(), month: d.getMonth() + 1 };
    });

  const days = data?.days ?? [];
  // Heat intensity is relative to the biggest day in the month.
  const maxAbs = Math.max(1, ...days.map((d) => Math.abs(d.profit)));

  return (
    <Section
      title="Calendar"
      description="Daily realized P&L, heat-mapped by size."
    >
      <div className="rounded-2xl border bg-background/90 p-4 shadow-sm">
        <div className="mb-3 flex items-center justify-between gap-2">
          <div className="flex items-center gap-1">
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label="Previous month"
              onClick={() => step(-1)}
            >
              <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={2} />
            </Button>
            <span className="min-w-[9rem] text-center text-sm font-semibold text-foreground">
              {MONTHS[cursor.month - 1]} {cursor.year}
            </span>
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label="Next month"
              onClick={() => step(1)}
            >
              <HugeiconsIcon
                icon={ArrowRight01Icon}
                size={16}
                strokeWidth={2}
              />
            </Button>
          </div>
          {data ? (
            <span
              className={cn(
                "text-sm font-semibold tabular-nums",
                data.monthProfit > 0 && "text-emerald-600",
                data.monthProfit < 0 && "text-rose-600",
              )}
            >
              {formatCurrency(data.monthProfit)} · {formatInt(data.tradingDays)}{" "}
              trading days
            </span>
          ) : null}
        </div>

        <div className="grid grid-cols-7 gap-1 text-center text-[0.6rem] font-semibold uppercase tracking-wide text-muted-foreground">
          {WEEKDAYS.map((w) => (
            <div key={w} className="py-1">
              {w}
            </div>
          ))}
        </div>

        <div
          className={cn(
            "grid grid-cols-7 gap-1 transition-opacity",
            isPlaceholderData && "opacity-60",
          )}
        >
          {days.map((day) => {
            // Parse from the string to stay timezone-safe.
            const dayNum = Number(day.date.slice(8, 10));
            const inMonth = Number(day.date.slice(5, 7)) === cursor.month;
            const traded = day.tradeCount > 0;
            const intensity = Math.min(1, Math.abs(day.profit) / maxAbs);
            const bg = traded
              ? day.profit >= 0
                ? `rgba(16,185,129,${0.14 + 0.5 * intensity})`
                : `rgba(244,63,94,${0.14 + 0.5 * intensity})`
              : undefined;
            return (
              <div
                key={day.date}
                className={cn(
                  "min-h-[3.75rem] rounded-md border p-1.5 text-left",
                  !inMonth && "opacity-40",
                )}
                style={{ backgroundColor: bg }}
              >
                <div className="text-[0.6rem] font-medium text-muted-foreground">
                  {dayNum}
                </div>
                {traded ? (
                  <div
                    className={cn(
                      "mt-0.5 text-[0.7rem] font-semibold tabular-nums",
                      day.profit >= 0 ? "text-emerald-700" : "text-rose-700",
                    )}
                  >
                    {formatCurrency(day.profit)}
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      </div>
    </Section>
  );
}
