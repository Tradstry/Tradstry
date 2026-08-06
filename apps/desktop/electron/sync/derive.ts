export type DerivedMetrics = {
  status: "profit" | "loss";
  totalPl: number;
  netRoi: number;
  duration: number;
  riskReward: number | null;
};

/** Mirrors the server/Rust journal calculations so offline writes stay identical. */
export function deriveMetrics(
  entry: number,
  exit: number,
  tradeType: string,
  open: string,
  close: string,
  stop: number | null,
): DerivedMetrics {
  const openMillis = parseFlexibleDatetime(open);
  const closeMillis = parseFlexibleDatetime(close);
  if (closeMillis < openMillis) throw new Error("close_date must be on or after open_date");

  let plRatio: number;
  if (tradeType === "long") plRatio = (exit - entry) / entry;
  else if (tradeType === "short") plRatio = (entry - exit) / entry;
  else throw new Error(`Unsupported trade_type: ${tradeType}`);

  let riskReward: number | null = null;
  if (stop !== null) {
    const riskDistance = tradeType === "long" ? entry - stop : stop - entry;
    if (riskDistance <= 0) {
      throw new Error(
        "stop_loss must be below entry_price for long trades and above entry_price for short trades",
      );
    }
    const rewardDistance = tradeType === "long" ? exit - entry : entry - exit;
    riskReward = rewardDistance / riskDistance;
  }

  const totalPl = plRatio * 100;
  return {
    status: totalPl < 0 ? "loss" : "profit",
    totalPl,
    netRoi: totalPl,
    duration: Math.trunc((closeMillis - openMillis) / 1000),
    riskReward,
  };
}

export function parseFlexibleDatetime(value: string): number {
  const rfc3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;
  if (rfc3339.test(value)) {
    const millis = Date.parse(value);
    if (Number.isFinite(millis)) return millis;
  }

  const match = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?)?$/.exec(value);
  if (match) {
    const [, yearText, monthText, dayText, hourText = "0", minuteText = "0", secondText = "0", fraction = ""] = match;
    const year = Number(yearText);
    const month = Number(monthText);
    const day = Number(dayText);
    const hour = Number(hourText);
    const minute = Number(minuteText);
    const second = Number(secondText);
    const milliseconds = Number(`${fraction}000`.slice(0, 3));
    const timestamp = Date.UTC(year, month - 1, day, hour, minute, second, milliseconds);
    const date = new Date(timestamp);
    if (
      date.getUTCFullYear() === year &&
      date.getUTCMonth() === month - 1 &&
      date.getUTCDate() === day &&
      date.getUTCHours() === hour &&
      date.getUTCMinutes() === minute &&
      date.getUTCSeconds() === second
    ) return timestamp;
  }
  throw new Error("Invalid datetime format. Use RFC3339, YYYY-MM-DD HH:MM[:SS], or YYYY-MM-DD");
}
