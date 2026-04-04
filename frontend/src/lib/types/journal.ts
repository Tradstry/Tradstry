export const JOURNAL_STATUSES = ["profit", "loss"] as const;
export const TRADE_TYPES = ["long", "short"] as const;

export type JournalStatus = (typeof JOURNAL_STATUSES)[number];
export type TradeType = (typeof TRADE_TYPES)[number];

export interface JournalEntry {
  id: string;
  userId: string;
  accountId: string;
  reviewed: boolean;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  symbol: string;
  symbolName: string;
  status: JournalStatus;
  totalPl: number;
  netRoi: number;
  duration: number;
  stopLoss: number;
  riskReward: number;
  tradeType: TradeType;
  mistakes: string;
  entryTactics: string;
  edgesSpotted: string;
  playbookId: string | null;
  notes: string | null;
}

export interface CreateJournalEntryInput {
  accountId: string;
  reviewed?: boolean;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  symbol: string;
  symbolName?: string | null;
  stopLoss: number;
  tradeType: TradeType;
  mistakes: string;
  entryTactics: string;
  edgesSpotted: string;
  playbookId?: string | null;
  notes?: string | null;
  brokerageTransactionIds?: string[];
}

export interface UpdateJournalEntryInput {
  accountId?: string;
  reviewed?: boolean;
  openDate?: string;
  closeDate?: string;
  entryPrice?: number;
  exitPrice?: number;
  positionSize?: number;
  symbol?: string;
  symbolName?: string | null;
  stopLoss?: number;
  tradeType?: TradeType;
  mistakes?: string;
  entryTactics?: string;
  edgesSpotted?: string;
  playbookId?: string | null;
  notes?: string | null;
  clearNotes?: boolean;
  clearPlaybook?: boolean;
}
