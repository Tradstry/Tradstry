import type { GraphQLFetcher } from "@/lib/client";
import type {
  BrokerageBalance,
  BrokerageHolding,
  BrokerageTransaction,
  BrokerageTransactionsPage,
  ConnectionPortal,
  LinkSnaptradeInput,
  PendingTrade,
  SyncResult,
  TransactionFilters,
} from "@/lib/types/brokerage";

// ---------------------------------------------------------------------------
// Field fragments
// ---------------------------------------------------------------------------

const TRANSACTION_FIELDS = `
  id
  userId
  accountId
  snaptradeId
  symbol
  symbolDescription
  rawSymbol
  currency
  transactionType
  optionType
  price
  units
  amount
  fee
  fxRate
  description
  tradeDate
  settlementDate
  institution
  externalReferenceId
  rawJson
  createdAt
  updatedAt
`;

const HOLDING_FIELDS = `
  id
  userId
  accountId
  snaptradeSymbolId
  symbol
  symbolDescription
  rawSymbol
  currency
  units
  price
  marketValue
  openPnl
  averagePurchasePrice
  isOption
  optionType
  strikePrice
  expirationDate
  rawJson
  syncedAt
  createdAt
  updatedAt
`;

const BALANCE_FIELDS = `
  id
  userId
  accountId
  currency
  cash
  buyingPower
  syncedAt
  createdAt
  updatedAt
`;

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

const BROKERAGE_TRANSACTIONS_QUERY = `
  query BrokerageTransactions(
    $accountId: String!
    $startDate: String
    $endDate: String
    $transactionType: String
    $symbol: String
    $offset: Int
    $limit: Int
    $sortBy: String
    $isJournalled: Boolean
  ) {
    brokerageTransactions(
      accountId: $accountId
      startDate: $startDate
      endDate: $endDate
      transactionType: $transactionType
      symbol: $symbol
      offset: $offset
      limit: $limit
      sortBy: $sortBy
      isJournalled: $isJournalled
    ) {
      data { ${TRANSACTION_FIELDS} }
      total
      offset
      limit
    }
  }
`;

const BROKERAGE_TRANSACTION_QUERY = `
  query BrokerageTransaction($id: String!) {
    brokerageTransaction(id: $id) {
      ${TRANSACTION_FIELDS}
    }
  }
`;

const BROKERAGE_HOLDINGS_QUERY = `
  query BrokerageHoldings($accountId: String!) {
    brokerageHoldings(accountId: $accountId) {
      ${HOLDING_FIELDS}
    }
  }
`;

const BROKERAGE_BALANCES_QUERY = `
  query BrokerageBalances($accountId: String!) {
    brokerageBalances(accountId: $accountId) {
      ${BALANCE_FIELDS}
    }
  }
`;

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

const INITIATE_CONNECTION_MUTATION = `
  mutation InitiateBrokerageConnection($accountId: String!, $brokerageId: String, $customRedirect: String) {
    initiateBrokerageConnection(accountId: $accountId, brokerageId: $brokerageId, customRedirect: $customRedirect) {
      redirectUrl
      connectionId
      snaptradeUserId
      snaptradeUserSecret
    }
  }
`;

const COMPLETE_CONNECTION_MUTATION = `
  mutation CompleteBrokerageConnection($accountId: String!, $connectionId: String!) {
    completeBrokerageConnection(accountId: $accountId, connectionId: $connectionId)
  }
`;

const LINK_SNAPTRADE_MUTATION = `
  mutation LinkSnaptradeAccount(
    $accountId: String!
    $snaptradeUserId: String!
    $snaptradeUserSecret: String!
    $snaptradeConnectionId: String
  ) {
    linkSnaptradeAccount(
      accountId: $accountId
      snaptradeUserId: $snaptradeUserId
      snaptradeUserSecret: $snaptradeUserSecret
      snaptradeConnectionId: $snaptradeConnectionId
    )
  }
`;

const DISCONNECT_BROKERAGE_MUTATION = `
  mutation DisconnectBrokerage($accountId: String!) {
    disconnectBrokerage(accountId: $accountId)
  }
`;

const SYNC_BROKERAGE_DATA_MUTATION = `
  mutation SyncBrokerageData($accountId: String!) {
    syncBrokerageData(accountId: $accountId) {
      transactionsSynced
      holdingsSynced
      balancesSynced
    }
  }
`;

// ---------------------------------------------------------------------------
// Service functions
// ---------------------------------------------------------------------------

export async function fetchTransactions(
  fetcher: GraphQLFetcher,
  accountId: string,
  filters?: TransactionFilters,
): Promise<BrokerageTransactionsPage> {
  const data = await fetcher<{
    brokerageTransactions: BrokerageTransactionsPage;
  }>(BROKERAGE_TRANSACTIONS_QUERY, { accountId, ...filters });
  return data.brokerageTransactions;
}

export async function fetchTransaction(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<BrokerageTransaction | null> {
  const data = await fetcher<{
    brokerageTransaction: BrokerageTransaction | null;
  }>(BROKERAGE_TRANSACTION_QUERY, { id });
  return data.brokerageTransaction;
}

export async function fetchHoldings(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<BrokerageHolding[]> {
  const data = await fetcher<{ brokerageHoldings: BrokerageHolding[] }>(
    BROKERAGE_HOLDINGS_QUERY,
    { accountId },
  );
  return data.brokerageHoldings;
}

export async function fetchBalances(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<BrokerageBalance[]> {
  const data = await fetcher<{ brokerageBalances: BrokerageBalance[] }>(
    BROKERAGE_BALANCES_QUERY,
    { accountId },
  );
  return data.brokerageBalances;
}

export async function initiateConnection(
  fetcher: GraphQLFetcher,
  accountId: string,
  brokerageId?: string,
  customRedirect?: string,
): Promise<ConnectionPortal> {
  const data = await fetcher<{ initiateBrokerageConnection: ConnectionPortal }>(
    INITIATE_CONNECTION_MUTATION,
    { accountId, brokerageId, customRedirect },
  );
  return data.initiateBrokerageConnection;
}

export async function completeConnection(
  fetcher: GraphQLFetcher,
  accountId: string,
  connectionId: string,
): Promise<boolean> {
  const data = await fetcher<{ completeBrokerageConnection: boolean }>(
    COMPLETE_CONNECTION_MUTATION,
    { accountId, connectionId },
  );
  return data.completeBrokerageConnection;
}

export async function linkSnaptradeAccount(
  fetcher: GraphQLFetcher,
  input: LinkSnaptradeInput,
): Promise<boolean> {
  const data = await fetcher<{ linkSnaptradeAccount: boolean }>(
    LINK_SNAPTRADE_MUTATION,
    { ...input },
  );
  return data.linkSnaptradeAccount;
}

export async function disconnectBrokerage(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<boolean> {
  const data = await fetcher<{ disconnectBrokerage: boolean }>(
    DISCONNECT_BROKERAGE_MUTATION,
    { accountId },
  );
  return data.disconnectBrokerage;
}

export async function syncBrokerageData(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<SyncResult> {
  const data = await fetcher<{ syncBrokerageData: SyncResult }>(
    SYNC_BROKERAGE_DATA_MUTATION,
    { accountId },
  );
  return data.syncBrokerageData;
}

const LINKED_BROKERAGE_TX_IDS_QUERY = `
  query LinkedBrokerageTransactionIds($accountId: String!) {
    linkedBrokerageTransactionIds(accountId: $accountId)
  }
`;

export async function fetchLinkedBrokerageTransactionIds(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<string[]> {
  const data = await fetcher<{ linkedBrokerageTransactionIds: string[] }>(
    LINKED_BROKERAGE_TX_IDS_QUERY,
    { accountId },
  );
  return data.linkedBrokerageTransactionIds;
}

const PENDING_TRADES_QUERY = `
  query PendingTrades($accountId: String!) {
    pendingTrades(accountId: $accountId) {
      id
      symbol
      direction
      status
      openDate
      closeDate
      entryUnits
      avgEntryPrice
      avgExitPrice
      realizedPnl
      transactionIds
      fillCount
      isFullyLinked
      isPartiallyLinked
    }
  }
`;

export async function fetchPendingTrades(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<PendingTrade[]> {
  const data = await fetcher<{ pendingTrades: PendingTrade[] }>(
    PENDING_TRADES_QUERY,
    { accountId },
  );
  return data.pendingTrades;
}

const BROKERAGE_TX_BY_IDS_QUERY = `
  query BrokerageTransactionsByIds($ids: [String!]!) {
    brokerageTransactionsByIds(ids: $ids) {
      ${TRANSACTION_FIELDS}
    }
  }
`;

export async function fetchBrokerageTransactionsByIds(
  fetcher: GraphQLFetcher,
  ids: string[],
): Promise<BrokerageTransaction[]> {
  if (ids.length === 0) return [];
  const data = await fetcher<{
    brokerageTransactionsByIds: BrokerageTransaction[];
  }>(BROKERAGE_TX_BY_IDS_QUERY, { ids });
  return data.brokerageTransactionsByIds;
}
