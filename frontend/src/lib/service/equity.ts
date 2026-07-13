import type { GraphQLFetcher } from "@/lib/client";
import type { AccountEquityHistory } from "@/lib/types/equity";

const EQUITY_HISTORY_FIELDS = `
  accountId
  points {
    date
    cash
    positionsValue
    equity
    netContributions
    fundingAdjustedEquity
  }
  health {
    rebuiltAt
    reconstructedEquity
    reportedEquity
    drift
    unclassifiedTypes
    excludedOptionTxns
    foreignCurrencyTxns
    missingPriceDays
  }
`;

const ACCOUNT_EQUITY_HISTORY_QUERY = `
  query AccountEquityHistory($accountId: String!, $from: String) {
    accountEquityHistory(accountId: $accountId, from: $from) {
      ${EQUITY_HISTORY_FIELDS}
    }
  }
`;

const REBUILD_ACCOUNT_EQUITY_HISTORY_MUTATION = `
  mutation RebuildAccountEquityHistory($accountId: String!) {
    rebuildAccountEquityHistory(accountId: $accountId) {
      ${EQUITY_HISTORY_FIELDS}
    }
  }
`;

export async function fetchAccountEquityHistory(
  fetcher: GraphQLFetcher,
  accountId: string,
  from?: string | null,
): Promise<AccountEquityHistory> {
  const data = await fetcher<{ accountEquityHistory: AccountEquityHistory }>(
    ACCOUNT_EQUITY_HISTORY_QUERY,
    { accountId, from: from ?? null },
  );
  return data.accountEquityHistory;
}

export async function rebuildAccountEquityHistory(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<AccountEquityHistory> {
  const data = await fetcher<{
    rebuildAccountEquityHistory: AccountEquityHistory;
  }>(REBUILD_ACCOUNT_EQUITY_HISTORY_MUTATION, { accountId });
  return data.rebuildAccountEquityHistory;
}
