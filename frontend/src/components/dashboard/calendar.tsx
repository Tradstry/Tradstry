"use client";

import { Fragment, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useActiveWorkspace } from "@/components/workspaces";
import { useCalendarAnalytics } from "@/hooks/analytics";
import { cn } from "@/lib/utils";

// UTC, because `visibleMonth` is midnight UTC on the 1st and every other read of
// it here is `getUTC*`. Formatting in local time renders the previous month for
// anyone west of UTC.
const monthFormatter = new Intl.DateTimeFormat("en-US", {
  month: "long",
  year: "numeric",
  timeZone: "UTC",
});

import { formatPnl } from "@/lib/utils";

const weekdayLabels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

function formatCurrency(value: number) {
  return formatPnl(value);
}

function toIsoDate(date: Date) {
  return date.toISOString().slice(0, 10);
}

function parseIsoDate(value: string) {
  return new Date(`${value}T00:00:00Z`);
}

function buildGridDates(start: string, end: string) {
  const dates: string[] = [];
  let cursor = parseIsoDate(start);
  const endDate = parseIsoDate(end);

  while (cursor <= endDate) {
    dates.push(toIsoDate(cursor));
    cursor = new Date(cursor.getTime() + 86_400_000);
  }

  return dates;
}

export function DashboardCalendar() {
  const activeWorkspace = useActiveWorkspace();
  const [visibleMonth, setVisibleMonth] = useState(() => {
    const now = new Date();
    return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1));
  });

  const year = visibleMonth.getUTCFullYear();
  const month = visibleMonth.getUTCMonth() + 1;
  const { data, isLoading, isPending, isPlaceholderData, error } =
    useCalendarAnalytics(activeWorkspace?.id ?? null, year, month);

  const monthLabel = monthFormatter.format(visibleMonth);
  const rows = useMemo(() => {
    if (!data) return [];

    const dates = buildGridDates(data.gridStart, data.gridEnd);
    return data.weeks.map((week, index) => ({
      week,
      dates: dates.slice(index * 7, index * 7 + 7),
    }));
  }, [data]);

  if (!activeWorkspace) {
    return (
      <section className="rounded-2xl border bg-background/80 p-6">
        <p className="text-sm font-medium text-foreground">
          No active workspace
        </p>
        <p className="mt-2 text-sm text-muted-foreground">
          Select a workspace to load the trading calendar.
        </p>
      </section>
    );
  }

  if (isLoading || isPending) {
    return (
      <section className="space-y-4 rounded-2xl border bg-background/80 p-5">
        <div className="flex items-center justify-between">
          <div className="space-y-2">
            <Skeleton className="h-6 w-40" />
            <Skeleton className="h-4 w-64" />
          </div>
          <div className="flex gap-2">
            <Skeleton className="h-10 w-10 rounded-xl" />
            <Skeleton className="h-10 w-10 rounded-xl" />
          </div>
        </div>
        <div className="grid gap-3 md:grid-cols-7">
          {weekdayLabels.map((label) => (
            <Skeleton key={label} className="h-10 rounded-xl" />
          ))}
        </div>
        <Skeleton className="h-[32rem] rounded-2xl" />
      </section>
    );
  }

  if (error instanceof Error) {
    return (
      <section className="rounded-2xl border border-rose-200 bg-rose-50 p-6 text-rose-700 dark:border-rose-900 dark:bg-rose-950/50 dark:text-rose-300">
        <p className="text-sm font-semibold uppercase tracking-[0.2em]">
          Calendar Error
        </p>
        <p className="mt-2 text-sm">{error.message}</p>
      </section>
    );
  }

  if (!data) {
    return null;
  }

  const dayMap = new Map(data.days.map((day) => [day.date, day]));

  return (
    <section className="rounded-[2rem] border bg-background/90 p-5 shadow-sm">
      <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div className="flex items-center gap-3">
          <Button
            variant="outline"
            size="icon"
            className="rounded-xl"
            onClick={() =>
              setVisibleMonth(
                new Date(Date.UTC(year, visibleMonth.getUTCMonth() - 1, 1)),
              )
            }
          >
            ←
          </Button>
          <div>
            <p className="text-[0.68rem] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
              Trading Calendar
            </p>
            <h2 className="mt-1 text-2xl font-semibold text-foreground">
              {monthLabel}
            </h2>
          </div>
          <Button
            variant="outline"
            size="icon"
            className="rounded-xl"
            onClick={() =>
              setVisibleMonth(
                new Date(Date.UTC(year, visibleMonth.getUTCMonth() + 1, 1)),
              )
            }
          >
            →
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
          <span
            className={cn(
              "font-semibold",
              data.monthProfit >= 0
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-rose-600 dark:text-rose-400",
            )}
          >
            {formatCurrency(data.monthProfit)}
          </span>
          <span>{data.tradingDays} trading days</span>
          <span>{data.tradeCount} trades</span>
        </div>
      </div>

      <div
        className={cn(
          "mt-6 grid grid-cols-[repeat(7,minmax(0,1fr))_8.5rem] gap-3 transition-opacity duration-200",
          isPlaceholderData && "opacity-60",
        )}
      >
        {weekdayLabels.map((label) => (
          <div
            key={label}
            className="rounded-xl border bg-muted/30 px-3 py-2 text-center text-sm font-medium text-muted-foreground"
          >
            {label}
          </div>
        ))}
        <div className="rounded-xl border bg-muted/30 px-3 py-2 text-center text-sm font-medium text-muted-foreground">
          Weeks
        </div>

        {rows.map(({ week, dates }) => (
          <Fragment key={week.weekIndex}>
            {dates.map((date) => {
              const cell = dayMap.get(date);
              const dateValue = parseIsoDate(date);
              const isCurrentMonth = dateValue.getUTCMonth() + 1 === month;

              return (
                <div
                  key={date}
                  className={cn(
                    "min-h-28 rounded-2xl border p-3",
                    isCurrentMonth
                      ? "bg-background"
                      : "bg-muted/20 text-muted-foreground/60",
                    cell &&
                      cell.tradeCount > 0 &&
                      cell.profit >= 0 &&
                      "border-emerald-200 bg-emerald-50/60 dark:border-emerald-900 dark:bg-emerald-950/40",
                    cell &&
                      cell.tradeCount > 0 &&
                      cell.profit < 0 &&
                      "border-rose-200 bg-rose-50/60 dark:border-rose-900 dark:bg-rose-950/40",
                  )}
                >
                  <p className="text-sm font-medium">
                    {dateValue.getUTCDate()}
                  </p>
                  {cell && cell.tradeCount > 0 ? (
                    <div className="mt-5 space-y-1">
                      <p
                        className={cn(
                          "text-lg font-semibold",
                          cell.profit >= 0
                            ? "text-emerald-700 dark:text-emerald-300"
                            : "text-rose-700 dark:text-rose-300",
                        )}
                      >
                        {formatCurrency(cell.profit)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {cell.tradeCount}{" "}
                        {cell.tradeCount === 1 ? "trade" : "trades"}
                      </p>
                    </div>
                  ) : null}
                </div>
              );
            })}

            <div className="flex min-h-28 flex-col justify-between rounded-2xl border bg-muted/20 p-3">
              <div>
                <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  Week {week.weekIndex}
                </p>
                <p
                  className={cn(
                    "mt-4 text-xl font-semibold",
                    week.profit >= 0
                      ? "text-emerald-700 dark:text-emerald-300"
                      : "text-rose-700 dark:text-rose-300",
                  )}
                >
                  {formatCurrency(week.profit)}
                </p>
              </div>
              <p className="text-xs text-muted-foreground">
                {week.tradingDays} day{week.tradingDays === 1 ? "" : "s"}
              </p>
            </div>
          </Fragment>
        ))}
      </div>
    </section>
  );
}
