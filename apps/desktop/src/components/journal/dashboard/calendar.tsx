import { Fragment, useEffect, useMemo, useState } from "react";
import { CaretLeftIcon, CaretRightIcon } from "@phosphor-icons/react";
import { calendarAnalytics, type CalendarAnalytics } from "../../../backend";

const monthFormatter = new Intl.DateTimeFormat("en-US", {
  month: "long",
  year: "numeric",
});
const weekdayLabels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});
const signedUsd = (v: number) => `${v >= 0 ? "+" : ""}${usd.format(v)}`;

function toIsoDate(d: Date) {
  return d.toISOString().slice(0, 10);
}
function parseIsoDate(v: string) {
  return new Date(`${v}T00:00:00Z`);
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

const navBtn =
  "flex h-8 w-8 cursor-pointer items-center justify-center rounded-lg border border-zinc-200 bg-white text-zinc-600 outline-none hover:bg-zinc-50 active:scale-95 focus-visible:outline-2 focus-visible:outline-blue-500 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-300 dark:hover:bg-zinc-800";

export default function Calendar({ accountId }: { accountId: string }) {
  const [visibleMonth, setVisibleMonth] = useState(() => {
    const now = new Date();
    return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1));
  });
  const year = visibleMonth.getUTCFullYear();
  const month = visibleMonth.getUTCMonth() + 1;

  const [data, setData] = useState<CalendarAnalytics | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setState("loading");
    calendarAnalytics(accountId, year, month)
      .then((d) => {
        if (!cancelled) {
          setData(d);
          setState("ready");
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setState("error");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [accountId, year, month]);

  const rows = useMemo(() => {
    if (!data) return [];
    const dates = buildGridDates(data.gridStart, data.gridEnd);
    return data.weeks.map((week, i) => ({
      week,
      dates: dates.slice(i * 7, i * 7 + 7),
    }));
  }, [data]);

  const goPrev = () =>
    setVisibleMonth(new Date(Date.UTC(year, visibleMonth.getUTCMonth() - 1, 1)));
  const goNext = () =>
    setVisibleMonth(new Date(Date.UTC(year, visibleMonth.getUTCMonth() + 1, 1)));
  const monthLabel = monthFormatter.format(visibleMonth);
  const dayMap = new Map((data?.days ?? []).map((d) => [d.date, d]));

  return (
    <section className="rounded-2xl border border-zinc-200/80 bg-white/85 p-5 backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div className="flex items-center gap-3">
          <button
            type="button"
            aria-label="Previous month"
            onClick={goPrev}
            className={navBtn}
          >
            <CaretLeftIcon size={15} />
          </button>
          <div>
            <p className="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
              Trading Calendar
            </p>
            <h3 className="mt-0.5 text-lg font-semibold text-zinc-900 dark:text-zinc-50">
              {monthLabel}
            </h3>
          </div>
          <button
            type="button"
            aria-label="Next month"
            onClick={goNext}
            className={navBtn}
          >
            <CaretRightIcon size={15} />
          </button>
        </div>
        {data && (
          <div className="flex flex-wrap items-center gap-3 text-sm text-zinc-500 dark:text-zinc-400">
            <span
              className={`font-semibold tabular-nums ${data.monthProfit >= 0 ? "text-emerald-600 dark:text-emerald-400" : "text-red-600 dark:text-red-400"}`}
            >
              {signedUsd(data.monthProfit)}
            </span>
            <span>{data.tradingDays} trading days</span>
            <span>{data.tradeCount} trades</span>
          </div>
        )}
      </div>

      {state === "error" ? (
        <p className="mt-4 rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
          Couldn't load calendar: {error}
        </p>
      ) : state === "loading" ? (
        <div className="mt-6 h-96 animate-pulse rounded-xl bg-zinc-100/70 dark:bg-zinc-900/50" />
      ) : (
        <div className="mt-6 grid grid-cols-[repeat(7,minmax(0,1fr))_7rem] gap-2">
          {weekdayLabels.map((l) => (
            <div
              key={l}
              className="rounded-lg bg-zinc-100/60 px-2 py-1.5 text-center text-xs font-medium text-zinc-500 dark:bg-zinc-800/40 dark:text-zinc-400"
            >
              {l}
            </div>
          ))}
          <div className="rounded-lg bg-zinc-100/60 px-2 py-1.5 text-center text-xs font-medium text-zinc-500 dark:bg-zinc-800/40 dark:text-zinc-400">
            Weeks
          </div>

          {rows.map(({ week, dates }) => (
            <Fragment key={week.weekIndex}>
              {dates.map((date) => {
                const cell = dayMap.get(date);
                const dv = parseIsoDate(date);
                const isCurrentMonth = dv.getUTCMonth() + 1 === month;
                const hasTrades = cell != null && cell.tradeCount > 0;
                const positive = hasTrades && cell!.profit >= 0;
                const negative = hasTrades && cell!.profit < 0;
                return (
                  <div
                    key={date}
                    className={[
                      "min-h-24 rounded-xl border p-2.5",
                      isCurrentMonth
                        ? "border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900"
                        : "border-transparent bg-zinc-50 text-zinc-400 dark:bg-zinc-950/40 dark:text-zinc-600",
                      positive
                        ? "border-emerald-200 bg-emerald-50/60 dark:border-emerald-900 dark:bg-emerald-950/30"
                        : "",
                      negative
                        ? "border-red-200 bg-red-50/60 dark:border-red-900 dark:bg-red-950/30"
                        : "",
                    ].join(" ")}
                  >
                    <p className="text-xs font-medium">{dv.getUTCDate()}</p>
                    {hasTrades && (
                      <div className="mt-3">
                        <p
                          className={`text-sm font-semibold tabular-nums ${cell!.profit >= 0 ? "text-emerald-700 dark:text-emerald-300" : "text-red-700 dark:text-red-300"}`}
                        >
                          {signedUsd(cell!.profit)}
                        </p>
                        <p className="text-[11px] text-zinc-500 dark:text-zinc-400">
                          {cell!.tradeCount}{" "}
                          {cell!.tradeCount === 1 ? "trade" : "trades"}
                        </p>
                      </div>
                    )}
                  </div>
                );
              })}
              <div className="flex min-h-24 flex-col justify-between rounded-xl border border-zinc-200 bg-zinc-50 p-2.5 dark:border-zinc-800 dark:bg-zinc-950/40">
                <div>
                  <p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-zinc-500 dark:text-zinc-400">
                    Week {week.weekIndex}
                  </p>
                  <p
                    className={`mt-2 text-base font-semibold tabular-nums ${week.profit >= 0 ? "text-emerald-700 dark:text-emerald-300" : "text-red-700 dark:text-red-300"}`}
                  >
                    {signedUsd(week.profit)}
                  </p>
                </div>
                <p className="text-[11px] text-zinc-500 dark:text-zinc-400">
                  {week.tradingDays} day{week.tradingDays === 1 ? "" : "s"}
                </p>
              </div>
            </Fragment>
          ))}
        </div>
      )}
    </section>
  );
}
