// Position-sizing math, ported verbatim from the web calculator
// (apps/website/src/components/position-calculator.tsx `calculate()` /
// `getStopLossError()`). Runs entirely client-side — no network.

export type PositionType = "long" | "short";

export type SizingInputs = {
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  positionType: PositionType;
};

export type SizingResult = {
  shares: number;
  positionValue: number;
  accountPct: number;
  stopLossPct: number;
  riskAmount: number;
};

export type PlanSeed = {
  symbol: string;
  positionType: PositionType;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  totalShares: number;
  positionValue: number;
};

export function getStopLossError(
  entryPrice: number,
  stopLoss: number,
  positionType: PositionType,
): string | null {
  if (!Number.isFinite(entryPrice) || !Number.isFinite(stopLoss)) return null;
  if (positionType === "long" && stopLoss >= entryPrice)
    return "Stop loss must be below entry price for a long position.";
  if (positionType === "short" && stopLoss <= entryPrice)
    return "Stop loss must be above entry price for a short position.";
  return null;
}

export function calculatePositionSize(
  inputs: SizingInputs,
): SizingResult | null {
  const {
    entryPrice: entry,
    stopLoss: stop,
    accountBalance: balance,
    accountRisk: risk,
    positionType,
  } = inputs;

  if (
    !Number.isFinite(entry) ||
    !Number.isFinite(stop) ||
    !Number.isFinite(balance) ||
    !Number.isFinite(risk) ||
    balance <= 0 ||
    risk <= 0 ||
    entry <= 0 ||
    stop <= 0
  )
    return null;

  if (positionType === "long" && stop >= entry) return null;
  if (positionType === "short" && stop <= entry) return null;

  const stopDistance = Math.abs(entry - stop);
  if (stopDistance === 0) return null;

  const riskAmount = balance * (risk / 100);
  const shares = riskAmount / stopDistance;
  const positionValue = shares * entry;
  const accountPct = (positionValue / balance) * 100;
  const stopLossPct = (stopDistance / entry) * 100;

  return { shares, positionValue, accountPct, stopLossPct, riskAmount };
}

export function fmt(n: number, decimals = 2): string {
  return n.toLocaleString("en-US", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}
