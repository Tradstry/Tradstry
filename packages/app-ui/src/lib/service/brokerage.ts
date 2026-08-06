import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
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
} from "@tradstry/app-ui/lib/types/brokerage";

// ---------------------------------------------------------------------------
// Field fragments
// ---------------------------------------------------------------------------

const TRANSACTION_FIELDS = `
  id
  userId
  workspaceId
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
  contractMultiplier
  underlyingSymbol
  optionKind
  strikePrice
  optionExpiration
  createdAt
  updatedAt
`;

const HOLDING_FIELDS = `
  id
  userId
  workspaceId
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
  syncedAt
  createdAt
  updatedAt
`;

const BALANCE_FIELDS = `
  id
  userId
  workspaceId
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
    $workspaceId: String!
    $startDate: String
    $endDate: String
    $range: AnalyticsRange
    $transactionType: String
    $symbol: String
    $offset: Int
    $limit: Int
    $sortBy: String
    $isJournalled: Boolean
  ) {
    brokerageTransactions(
      workspaceId: $workspaceId
      startDate: $startDate
      endDate: $endDate
      range: $range
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
  query BrokerageHoldings($workspaceId: String!) {
    brokerageHoldings(workspaceId: $workspaceId) {
      ${HOLDING_FIELDS}
    }
  }
`;

const BROKERAGE_BALANCES_QUERY = `
  query BrokerageBalances($workspaceId: String!) {
    brokerageBalances(workspaceId: $workspaceId) {
      ${BALANCE_FIELDS}
    }
  }
`;

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

const INITIATE_CONNECTION_MUTATION = `
  mutation InitiateBrokerageConnection($workspaceId: String!, $brokerageId: String, $customRedirect: String, $reconnect: Boolean) {
    initiateBrokerageConnection(workspaceId: $workspaceId, brokerageId: $brokerageId, customRedirect: $customRedirect, reconnect: $reconnect) {
      redirectUrl
      connectionId
      snaptradeUserId
      snaptradeUserSecret
    }
  }
`;

const COMPLETE_CONNECTION_MUTATION = `
  mutation CompleteBrokerageConnection($workspaceId: String!, $connectionId: String!) {
    completeBrokerageConnection(workspaceId: $workspaceId, connectionId: $connectionId)
  }
`;

const LINK_SNAPTRADE_MUTATION = `
  mutation LinkSnaptradeAccount(
    $workspaceId: String!
    $snaptradeUserId: String!
    $snaptradeUserSecret: String!
    $snaptradeConnectionId: String
  ) {
    linkSnaptradeAccount(
      workspaceId: $workspaceId
      snaptradeUserId: $snaptradeUserId
      snaptradeUserSecret: $snaptradeUserSecret
      snaptradeConnectionId: $snaptradeConnectionId
    )
  }
`;

const DISCONNECT_BROKERAGE_MUTATION = `
  mutation DisconnectBrokerage($workspaceId: String!) {
    disconnectBrokerage(workspaceId: $workspaceId)
  }
`;

const SYNC_BROKERAGE_DATA_MUTATION = `
  mutation SyncBrokerageData($workspaceId: String!) {
    syncBrokerageData(workspaceId: $workspaceId) {
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
  workspaceId: string,
  filters?: TransactionFilters,
): Promise<BrokerageTransactionsPage> {
  const data = await fetcher<{
    brokerageTransactions: BrokerageTransactionsPage;
  }>(BROKERAGE_TRANSACTIONS_QUERY, { workspaceId, ...filters });
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
  workspaceId: string,
): Promise<BrokerageHolding[]> {
  const data = await fetcher<{ brokerageHoldings: BrokerageHolding[] }>(
    BROKERAGE_HOLDINGS_QUERY,
    { workspaceId },
  );
  return data.brokerageHoldings;
}

export async function fetchBalances(
  fetcher: GraphQLFetcher,
  workspaceId: string,
): Promise<BrokerageBalance[]> {
  const data = await fetcher<{ brokerageBalances: BrokerageBalance[] }>(
    BROKERAGE_BALANCES_QUERY,
    { workspaceId },
  );
  return data.brokerageBalances;
}

export async function initiateConnection(
  fetcher: GraphQLFetcher,
  workspaceId: string,
  brokerageId?: string,
  customRedirect?: string,
  reconnect?: boolean,
): Promise<ConnectionPortal> {
  const data = await fetcher<{ initiateBrokerageConnection: ConnectionPortal }>(
    INITIATE_CONNECTION_MUTATION,
    { workspaceId, brokerageId, customRedirect, reconnect },
  );
  return data.initiateBrokerageConnection;
}

export async function completeConnection(
  fetcher: GraphQLFetcher,
  workspaceId: string,
  connectionId: string,
): Promise<boolean> {
  const data = await fetcher<{ completeBrokerageConnection: boolean }>(
    COMPLETE_CONNECTION_MUTATION,
    { workspaceId, connectionId },
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
  workspaceId: string,
): Promise<boolean> {
  const data = await fetcher<{ disconnectBrokerage: boolean }>(
    DISCONNECT_BROKERAGE_MUTATION,
    { workspaceId },
  );
  return data.disconnectBrokerage;
}

export async function syncBrokerageData(
  fetcher: GraphQLFetcher,
  workspaceId: string,
): Promise<SyncResult> {
  const data = await fetcher<{ syncBrokerageData: SyncResult }>(
    SYNC_BROKERAGE_DATA_MUTATION,
    { workspaceId },
  );
  return data.syncBrokerageData;
}

const LINKED_BROKERAGE_TX_IDS_QUERY = `
  query LinkedBrokerageTransactionIds($workspaceId: String!) {
    linkedBrokerageTransactionIds(workspaceId: $workspaceId)
  }
`;

export async function fetchLinkedBrokerageTransactionIds(
  fetcher: GraphQLFetcher,
  workspaceId: string,
): Promise<string[]> {
  const data = await fetcher<{ linkedBrokerageTransactionIds: string[] }>(
    LINKED_BROKERAGE_TX_IDS_QUERY,
    { workspaceId },
  );
  return data.linkedBrokerageTransactionIds;
}

const PENDING_TRADES_QUERY = `
  query PendingTrades($workspaceId: String!) {
    pendingTrades(workspaceId: $workspaceId) {
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
      multiplier
      isOption
      underlying
      optionKind
      strike
      expiration
      symbolName
    }
  }
`;

export async function fetchPendingTrades(
  fetcher: GraphQLFetcher,
  workspaceId: string,
): Promise<PendingTrade[]> {
  const data = await fetcher<{ pendingTrades: PendingTrade[] }>(
    PENDING_TRADES_QUERY,
    { workspaceId },
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
