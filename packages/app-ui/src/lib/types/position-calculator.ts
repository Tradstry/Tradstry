export interface PositionCalculatorRule {
	id: string;
	userId: string;
	workspaceId: string;
	accountBalance: number;
	accountRisk: number;
	maxStopLossPct: number;
	createdAt: string;
	updatedAt: string;
}

export interface UpsertPositionCalculatorRuleInput {
	workspaceId: string;
	accountBalance: number;
	accountRisk: number;
	maxStopLossPct: number;
}

export interface PositionCalculatorHistoryEntry {
	id: string;
	userId: string;
	workspaceId: string;
	symbol: string;
	positionType: string;
	entryPrice: number;
	stopLoss: number;
	accountBalance: number;
	accountRisk: number;
	shares: number;
	positionValue: number;
	accountPct: number;
	stopLossPct: number;
	planId: string | null;
	tranches: HistoryTranche[];
	createdAt: string;
}

export interface HistoryTranche {
	id: string;
	percent: number;
	shares: number;
	targetPrice: number;
	status: string;
	filledAt: string | null;
}

export interface CreateHistoryTrancheInput {
	id: string;
	percent: number;
	shares: number;
	targetPrice: number;
	status: string;
	filledAt?: string | null;
}

export interface CreatePositionCalculatorHistoryInput {
	workspaceId: string;
	symbol: string;
	positionType: string;
	entryPrice: number;
	stopLoss: number;
	accountBalance: number;
	accountRisk: number;
	shares: number;
	positionValue: number;
	accountPct: number;
	stopLossPct: number;
	planId?: string | null;
	tranches?: CreateHistoryTrancheInput[];
}

export interface Tranche {
	id: string;
	percent: number;
	shares: number;
	targetPrice: number;
	status: string;
	filledAt: string | null;
}

export interface PositionCalculatorPlan {
	id: string;
	userId: string;
	workspaceId: string;
	symbol: string;
	positionType: string;
	entryPrice: number;
	stopLoss: number;
	accountBalance: number;
	accountRisk: number;
	totalShares: number;
	positionValue: number;
	status: string;
	tranches: Tranche[];
	notes: string | null;
	instrumentJson: string | null;
	createdAt: string;
	updatedAt: string;
}

export interface CreateTrancheInput {
	percent: number;
	shares: number;
	targetPrice: number;
}

export interface CreatePositionCalculatorPlanInput {
	workspaceId: string;
	symbol: string;
	positionType: string;
	entryPrice: number;
	stopLoss: number;
	accountBalance: number;
	accountRisk: number;
	totalShares: number;
	positionValue: number;
	tranches: CreateTrancheInput[];
	notes?: string | null;
	instrumentJson?: string | null;
}

export interface UpdateTrancheInput {
	id: string;
	percent?: number;
	shares?: number;
	targetPrice?: number;
	status?: string;
}

export interface UpdatePositionCalculatorPlanInput {
	status?: string;
	tranches?: UpdateTrancheInput[];
	notes?: string | null;
	clearNotes?: boolean;
}

export interface TradeReviewMatchSuggestion {
	matchId: string;
	planId: string;
	score: string;
	evidence: {
		time_delta_minutes: number;
		planned_quantity: string;
		actual_quantity: string;
		quantity_delta: string;
		planned_entry: string | null;
		actual_entry: string;
	};
}

export interface TradeReviewInboxItem {
	episodeId: string;
	instrumentKey: string;
	direction: string;
	openedAt: string;
	closedAt: string | null;
	currentQuantity: string;
	status: string;
	blockReason: string | null;
	matchStatus: string | null;
	confirmedMatchId: string | null;
	confirmedPlanId: string | null;
	suggestionsJson: string;
	latestReviewJson: string | null;
}

export interface ManualExecutionClaim {
	id: string;
	workspaceId: string;
	planId: string;
	trancheId: string;
	quantity: string;
	price: string;
	executedAt: string;
	status: "pending" | "reconciled";
	reconciledMatchId: string | null;
	createdAt: string;
}
