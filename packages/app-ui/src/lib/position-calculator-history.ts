import type {
	HistoryTranche,
	PositionCalculatorHistoryEntry,
	PositionCalculatorPlan,
} from "@tradstry/app-ui/lib/types/position-calculator";

const CLOSE_ENOUGH = 0.01;

function filledSummary(tranches: HistoryTranche[]) {
	const filled = tranches.filter((tranche) => tranche.status === "filled");
	const shares = filled.reduce((sum, tranche) => sum + tranche.shares, 0);
	const entryPrice =
		shares > 0
			? filled.reduce(
					(sum, tranche) => sum + tranche.shares * tranche.targetPrice,
					0,
				) / shares
			: 0;
	return { shares, entryPrice };
}

/**
 * New history entries contain their own immutable tranche snapshot. For
 * entries created before snapshots existed, recover details only when a
 * completed plan matches both the filled share count and weighted entry.
 */
export function resolveHistoryTranches(
	entry: PositionCalculatorHistoryEntry,
	plans: PositionCalculatorPlan[],
): HistoryTranche[] {
	if (entry.tranches.length > 0) return entry.tranches;

	const candidates = plans.filter(
		(plan) =>
			plan.status === "completed" &&
			plan.symbol === entry.symbol &&
			plan.positionType === entry.positionType &&
			(!entry.planId || plan.id === entry.planId),
	);

	const match = candidates.find((plan) => {
		const summary = filledSummary(plan.tranches);
		return (
			Math.abs(summary.shares - entry.shares) <= CLOSE_ENOUGH &&
			Math.abs(summary.entryPrice - entry.entryPrice) <= CLOSE_ENOUGH
		);
	});

	return match?.tranches ?? [];
}

export function trancheRisk(
	positionType: string,
	stopLoss: number,
	tranche: HistoryTranche,
): number | null {
	if (tranche.status !== "filled") return null;
	const riskPerShare =
		positionType === "short"
			? stopLoss - tranche.targetPrice
			: tranche.targetPrice - stopLoss;
	return riskPerShare > 0 ? riskPerShare * tranche.shares : null;
}
