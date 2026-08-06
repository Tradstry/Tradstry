export interface EquityHistoryPoint {
  date: string;
  cash: number;
  positionsValue: number;
  equity: number;
  netContributions: number;
  fundingAdjustedEquity: number;
}

export interface EquityHistoryHealth {
  rebuiltAt: string;
  reconstructedEquity: number | null;
  reportedEquity: number | null;
  drift: number | null;
  unclassifiedTypes: string[];
  excludedOptionTxns: number;
  foreignCurrencyTxns: number;
  missingPriceDays: number;
}

export interface AccountEquityHistory {
  workspaceId: string;
  points: EquityHistoryPoint[];
  health: EquityHistoryHealth | null;
}
