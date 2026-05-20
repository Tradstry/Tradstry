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
  accountId: string;
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
  rawJson: string;
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
  accountId: string;
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
  rawJson: string;
  syncedAt: string;
  createdAt: string;
  updatedAt: string;
}

export interface BrokerageBalance {
  id: string;
  userId: string;
  accountId: string;
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
  transactionType?: string;
  symbol?: string;
  sortBy?: string;
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
}

export interface LinkSnaptradeInput {
  accountId: string;
  snaptradeUserId: string;
  snaptradeUserSecret: string;
  snaptradeConnectionId?: string;
}
