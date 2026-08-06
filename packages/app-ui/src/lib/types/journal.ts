import type { Tag } from "@tradstry/app-ui/lib/types/tags";

export const JOURNAL_STATUSES = ["profit", "loss"] as const;
export const TRADE_TYPES = ["long", "short"] as const;

export type JournalStatus = (typeof JOURNAL_STATUSES)[number];
export type TradeType = (typeof TRADE_TYPES)[number];

export interface JournalEntry {
  id: string;
  userId: string;
  workspaceId: string;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  symbol: string;
  symbolName: string;
  /** 1 for equities; 100 (or 10 for minis) for option contracts. */
  contractMultiplier?: number;
  status: JournalStatus;
  totalPl: number;
  netRoi: number;
  duration: number;
  stopLoss: number;
  riskReward: number | null;
  tradeType: TradeType;
  /** Legacy freeform field — read-only; frozen on new writes. */
  mistakes?: string;
  /** Legacy freeform field — read-only; frozen on new writes. */
  entryTactics?: string;
  /** Legacy freeform field — read-only; frozen on new writes. */
  edgesSpotted?: string;
  /** Normalized tags attached to this trade. */
  tags: Tag[];
  playbookId: string | null;
  notes: string | null;
}

export interface CreateJournalEntryInput {
  workspaceId: string;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  symbol: string;
  symbolName?: string | null;
  stopLoss: number;
  tradeType: TradeType;
  tagIds: string[];
  violatedPrincipleIds: string[];
  playbookId?: string | null;
  notes?: string | null;
  brokerageTransactionIds?: string[];
  /** 1 for equities; 100 (or 10 for minis) for option contracts. Drives dollar P&L. */
  contractMultiplier?: number;
}

export interface UpdateJournalEntryInput {
  workspaceId?: string;
  openDate?: string;
  closeDate?: string;
  entryPrice?: number;
  exitPrice?: number;
  positionSize?: number;
  symbol?: string;
  symbolName?: string | null;
  stopLoss?: number;
  tradeType?: TradeType;
  /** Omit to leave tags unchanged; pass an empty array to clear all tags. */
  tagIds?: string[];
  /** Omit to leave violations unchanged; pass an empty array to clear them all. */
  violatedPrincipleIds?: string[];
  playbookId?: string | null;
  notes?: string | null;
  clearNotes?: boolean;
  clearPlaybook?: boolean;
}
