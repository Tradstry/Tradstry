import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

const FMT_USD_WHOLE = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 0,
});

const FMT_USD_CENTS = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

/**
 * Format a number as a P&L value with a leading sign (+/−) and the $ symbol.
 *
 * Behaviour:
 * - Zero-floor: any value whose absolute magnitude rounds to zero at the
 *   chosen precision renders as plain "$0" with no sign — never "-$0".
 * - precision = "auto" (default): shows 2 decimals when |value| < 1 so
 *   sub-dollar moves like $0.03 stay visible, and 0 decimals otherwise so
 *   normal-sized P&L stays scannable. Use this for dense surfaces like the
 *   calendar grid.
 * - precision = "cents": always 2 decimals. Use for stat cards / tables
 *   where consistent column alignment matters.
 */
export function formatPnl(
  value: number,
  opts: { precision?: "auto" | "cents" } = {},
): string {
  const precision = opts.precision ?? "auto";

  const useCents = precision === "cents" || Math.abs(value) < 1;
  const threshold = useCents ? 0.005 : 0.5;

  if (!Number.isFinite(value) || Math.abs(value) < threshold) {
    return useCents ? "$0.00" : "$0";
  }

  const formatter = useCents ? FMT_USD_CENTS : FMT_USD_WHOLE;
  const sign = value > 0 ? "+" : "";
  return `${sign}${formatter.format(value)}`;
}
