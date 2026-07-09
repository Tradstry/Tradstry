export interface PositionCalculatorRule {
  id: string;
  userId: string;
  accountId: string;
  accountBalance: number;
  accountRisk: number;
  maxStopLossPct: number;
  createdAt: string;
  updatedAt: string;
}

export interface UpsertPositionCalculatorRuleInput {
  accountId: string;
  accountBalance: number;
  accountRisk: number;
  maxStopLossPct: number;
}

export interface PositionCalculatorHistoryEntry {
  id: string;
  userId: string;
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
  createdAt: string;
}

export interface CreatePositionCalculatorHistoryInput {
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
  createdAt: string;
  updatedAt: string;
}

export interface CreateTrancheInput {
  percent: number;
  shares: number;
  targetPrice: number;
}

export interface CreatePositionCalculatorPlanInput {
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
