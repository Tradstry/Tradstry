import type { Tag } from "@/lib/types/tags";

export const JOURNAL_STATUSES = ["profit", "loss"] as const;
export const TRADE_TYPES = ["long", "short"] as const;

export type JournalStatus = (typeof JOURNAL_STATUSES)[number];
export type TradeType = (typeof TRADE_TYPES)[number];

export interface JournalEntry {
  id: string;
  userId: string;
  accountId: string;
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
  accountId: string;
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
  playbookId?: string | null;
  notes?: string | null;
  brokerageTransactionIds?: string[];
}

export interface UpdateJournalEntryInput {
  accountId?: string;
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
  playbookId?: string | null;
  notes?: string | null;
  clearNotes?: boolean;
  clearPlaybook?: boolean;
}
