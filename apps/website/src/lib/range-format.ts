import type { AnalyticsRange } from "@/lib/types/analytics";

/**
 * Lower-case human labels for each range preset. Used for inline sublabels
 * (e.g. "Net realized P/L · year to date").
 */
const RANGE_LABELS: Record<AnalyticsRange, string> = {
  TODAY: "today",
  LAST_7_DAYS: "past 7 days",
  LAST_1_MONTH: "past month",
  LAST_3_MONTHS: "past 3 months",
  LAST_6_MONTHS: "past 6 months",
  YEAR_TO_DATE: "year to date",
  LAST_1_YEAR: "past year",
  ALL: "all time",
  CUSTOM: "custom range",
};

const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

/** Lower-case label for a range, e.g. "year to date". */
export function rangeSublabel(range: AnalyticsRange): string {
  return RANGE_LABELS[range] ?? "selected range";
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Format one `YYYY-MM-DD` date as "Jun 20", optionally with the year. */
function formatDay(iso: string, withYear: boolean): string {
  const [year, month, day] = iso.split("-").map(Number);
  if (!year || !month || !day) return iso;
  const base = `${MONTHS[month - 1]} ${day}`;
  return withYear ? `${base}, ${year}` : base;
}

/**
 * Format a resolved span as "Jan 1 – Jun 20" (years appended only when the
 * endpoints fall in different years). Returns null when either bound is
 * missing (the unbounded "all" range).
 */
export function formatRangeSpan(
  start: string | null | undefined,
  end: string | null | undefined,
): string | null {
  if (!start || !end) return null;
  const crossesYears = start.slice(0, 4) !== end.slice(0, 4);
  return `${formatDay(start, crossesYears)} – ${formatDay(end, crossesYears)}`;
}

/**
 * Subtitle suffix: capitalized label plus the span in parentheses, e.g.
 * "Year to date (Jan 1 – Jun 20)". Falls back to the label alone when the
 * span is unavailable (unbounded range, or before the first load resolves).
 */
export function rangeSubtitleSuffix(
  range: AnalyticsRange,
  start: string | null | undefined,
  end: string | null | undefined,
): string {
  const label = capitalize(rangeSublabel(range));
  const span = formatRangeSpan(start, end);
  return span ? `${label} (${span})` : label;
}
