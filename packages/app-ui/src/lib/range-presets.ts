import type { AnalyticsRange } from "@tradstry/app-ui/lib/types/analytics";

/** Canonical, ET-anchored range presets shared across dashboard and brokerage. */
export const RANGE_PRESETS: Array<{ label: string; value: AnalyticsRange }> = [
  { label: "1D", value: "TODAY" },
  { label: "1W", value: "LAST_7_DAYS" },
  { label: "1M", value: "LAST_1_MONTH" },
  { label: "3M", value: "LAST_3_MONTHS" },
  { label: "6M", value: "LAST_6_MONTHS" },
  { label: "YTD", value: "YEAR_TO_DATE" },
  { label: "1Y", value: "LAST_1_YEAR" },
  { label: "Max", value: "ALL" },
];
