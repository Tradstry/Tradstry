import type { AnalyticsRange } from "@/lib/types/analytics";

export const TRANSACTION_TYPES = [
  "BUY",
  "SELL",
  "DIVIDEND",
  "CONTRIBUTION",
  "WITHDRAWAL",
  "REI",
  "STOCK_DIVIDEND",
  "INTEREST",
  "FEE",
  "TAX",
  "OPTIONEXPIRATION",
  "OPTIONASSIGNMENT",
  "OPTIONEXERCISE",
  "TRANSFER",
  "EXTERNAL_ASSET_TRANSFER_IN",
  "EXTERNAL_ASSET_TRANSFER_OUT",
  "SPLIT",
  "ADJUSTMENT",
] as const;

export type TransactionType = (typeof TRANSACTION_TYPES)[number];

export interface BrokerageTransaction {
  id: string;
  userId: string;
  workspaceId: string;
  snaptradeId: string;
  symbol: string | null;
  symbolDescription: string | null;
  rawSymbol: string | null;
  currency: string;
  transactionType: string;
  optionType: string | null;
  price: number;
  units: number;
  amount: number | null;
  fee: number;
  fxRate: number | null;
  description: string | null;
  tradeDate: string | null;
  settlementDate: string;
  institution: string;
  externalReferenceId: string | null;
  /** 1 for equities; 100 (or 10 for minis) for option contracts. */
  contractMultiplier: number;
  underlyingSymbol: string | null;
  /** "CALL" | "PUT" for options; null for equities. */
  optionKind: string | null;
  strikePrice: number | null;
  optionExpiration: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface BrokerageTransactionsPage {
  data: BrokerageTransaction[];
  total: number;
  offset: number;
  limit: number;
}

export interface BrokerageHolding {
  id: string;
  userId: string;
  workspaceId: string;
  snaptradeSymbolId: string | null;
  symbol: string;
  symbolDescription: string | null;
  rawSymbol: string | null;
  currency: string;
  units: number;
  price: number;
  marketValue: number | null;
  openPnl: number | null;
  averagePurchasePrice: number | null;
  isOption: boolean;
  optionType: string | null;
  strikePrice: number | null;
  expirationDate: string | null;
  syncedAt: string;
  createdAt: string;
  updatedAt: string;
}

export interface BrokerageBalance {
  id: string;
  userId: string;
  workspaceId: string;
  currency: string;
  cash: number | null;
  buyingPower: number | null;
  syncedAt: string;
  createdAt: string;
  updatedAt: string;
}

export interface TransactionFilters {
  startDate?: string;
  endDate?: string;
  /** ET-anchored preset; when set, the backend derives start/end dates. */
  range?: AnalyticsRange;
  transactionType?: string;
  symbol?: string;
  sortBy?: string;
  /** undefined = all; true = only journalled; false = only not-yet-journalled. */
  isJournalled?: boolean;
  offset?: number;
  limit?: number;
}

export interface SyncResult {
  transactionsSynced: number;
  holdingsSynced: number;
  balancesSynced: number;
}

export interface ConnectionPortal {
  redirectUrl: string;
  connectionId: string;
  snaptradeUserId: string;
  snaptradeUserSecret: string;
}

export interface PendingTrade {
  id: string;
  symbol: string;
  direction: "long" | "short";
  status: "open" | "closed";
  openDate: string;
  closeDate: string | null;
  entryUnits: number;
  avgEntryPrice: number;
  avgExitPrice: number | null;
  realizedPnl: number | null;
  transactionIds: string[];
  fillCount: number;
  isFullyLinked: boolean;
  isPartiallyLinked: boolean;
  /** 1 for equities; 100 (or 10 for minis) for option contracts. */
  multiplier: number;
  isOption: boolean;
  underlying: string | null;
  /** "CALL" | "PUT" for options; null for equities. */
  optionKind: string | null;
  strike: number | null;
  expiration: string | null;
  /** Human-readable contract name for options, e.g. "AAPL $150 Call 2026-01-17". */
  symbolName: string | null;
}

export interface LinkSnaptradeInput {
  workspaceId: string;
  snaptradeUserId: string;
  snaptradeUserSecret: string;
  snaptradeConnectionId?: string;
}
