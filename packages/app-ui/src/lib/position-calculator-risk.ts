export type TrancheRiskCalculation = {
	allocatedRisk: number;
	riskPerShare: number;
	shares: number;
	actualRisk: number;
	positionValue: number;
	stopDistancePct: number;
};

export type PlanRiskSummary = {
	totalShares: number;
	totalRisk: number;
	positionValue: number;
	weightedEntry: number;
};

function floorTo(value: number, decimals: number) {
	const factor = 10 ** decimals;
	return Math.floor((value + Number.EPSILON) * factor) / factor;
}

export function calculateRiskBudget(
	accountBalance: number,
	accountRisk: number,
) {
	if (
		!Number.isFinite(accountBalance) ||
		!Number.isFinite(accountRisk) ||
		accountBalance <= 0 ||
		accountRisk <= 0
	) {
		return null;
	}

	return accountBalance * (accountRisk / 100);
}

export function calculateTrancheRisk({
	positionType,
	entryPrice,
	stopLoss,
	riskBudget,
	riskPercent,
}: {
	positionType: string;
	entryPrice: number;
	stopLoss: number;
	riskBudget: number;
	riskPercent: number;
}): TrancheRiskCalculation | null {
	if (
		(positionType !== "long" && positionType !== "short") ||
		!Number.isFinite(entryPrice) ||
		!Number.isFinite(stopLoss) ||
		!Number.isFinite(riskBudget) ||
		!Number.isFinite(riskPercent) ||
		entryPrice <= 0 ||
		stopLoss <= 0 ||
		riskBudget <= 0 ||
		riskPercent <= 0
	) {
		return null;
	}

	const riskPerShare =
		positionType === "short" ? stopLoss - entryPrice : entryPrice - stopLoss;
	if (riskPerShare <= 0) return null;

	const allocatedRisk = riskBudget * (riskPercent / 100);
	// Never let fractional-share rounding push the planned loss over budget.
	const shares = floorTo(allocatedRisk / riskPerShare, 2);
	if (shares <= 0) return null;
	const actualRisk = shares * riskPerShare;

	return {
		allocatedRisk,
		riskPerShare,
		shares,
		actualRisk,
		positionValue: shares * entryPrice,
		stopDistancePct: (riskPerShare / entryPrice) * 100,
	};
}

export function summarizePlanRisk(
	tranches: Array<{
		shares: number;
		targetPrice: number;
		actualRisk: number;
	}>,
): PlanRiskSummary | null {
	if (tranches.length === 0) return null;

	const totalShares = tranches.reduce(
		(sum, tranche) => sum + tranche.shares,
		0,
	);
	const totalRisk = tranches.reduce(
		(sum, tranche) => sum + tranche.actualRisk,
		0,
	);
	const positionValue = tranches.reduce(
		(sum, tranche) => sum + tranche.shares * tranche.targetPrice,
		0,
	);

	if (totalShares <= 0) return null;

	return {
		totalShares,
		totalRisk,
		positionValue,
		weightedEntry: positionValue / totalShares,
	};
}
