import type { GraphQLFetcher } from "@/lib/client";
import type { AccountEquityHistory } from "@/lib/types/equity";

const EQUITY_HISTORY_FIELDS = `
  workspaceId
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
  query AccountEquityHistory($workspaceId: String!, $from: String) {
    accountEquityHistory(workspaceId: $workspaceId, from: $from) {
      ${EQUITY_HISTORY_FIELDS}
    }
  }
`;

const REBUILD_ACCOUNT_EQUITY_HISTORY_MUTATION = `
  mutation RebuildAccountEquityHistory($workspaceId: String!) {
    rebuildAccountEquityHistory(workspaceId: $workspaceId) {
      ${EQUITY_HISTORY_FIELDS}
    }
  }
`;

export async function fetchAccountEquityHistory(
  fetcher: GraphQLFetcher,
  workspaceId: string,
  from?: string | null,
): Promise<AccountEquityHistory> {
  const data = await fetcher<{ accountEquityHistory: AccountEquityHistory }>(
    ACCOUNT_EQUITY_HISTORY_QUERY,
    { workspaceId, from: from ?? null },
  );
  return data.accountEquityHistory;
}

export async function rebuildAccountEquityHistory(
  fetcher: GraphQLFetcher,
  workspaceId: string,
): Promise<AccountEquityHistory> {
  const data = await fetcher<{
    rebuildAccountEquityHistory: AccountEquityHistory;
  }>(REBUILD_ACCOUNT_EQUITY_HISTORY_MUTATION, { workspaceId });
  return data.rebuildAccountEquityHistory;
}
