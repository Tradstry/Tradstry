import type { GraphQLFetcher } from "@/lib/client";
import type { Account, CreateAccountInput, UpdateAccountInput } from "@/lib/types/accounts";

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

const ACCOUNTS_QUERY = `
  query Accounts {
    accounts {
      id
      userId
      name
      icon
      currency
      broker
      riskProfile
      createdAt
      updatedAt
      snaptradeUserId
      snaptradeConnectionId
      snaptradeConnectionDisabled
      snaptradeConnectionDisabledAt
    }
  }
`;

const ACCOUNT_QUERY = `
  query Account($id: String!) {
    account(id: $id) {
      id
      userId
      name
      icon
      currency
      broker
      riskProfile
      createdAt
      updatedAt
      snaptradeUserId
      snaptradeConnectionId
      snaptradeConnectionDisabled
      snaptradeConnectionDisabledAt
    }
  }
`;

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

const CREATE_ACCOUNT_MUTATION = `
  mutation CreateAccount($input: CreateAccountInput!) {
    createAccount(input: $input) {
      id
      userId
      name
      icon
      currency
      broker
      riskProfile
      createdAt
      updatedAt
      snaptradeUserId
      snaptradeConnectionId
      snaptradeConnectionDisabled
      snaptradeConnectionDisabledAt
    }
  }
`;

const UPDATE_ACCOUNT_MUTATION = `
  mutation UpdateAccount($id: String!, $input: UpdateAccountInput!) {
    updateAccount(id: $id, input: $input) {
      id
      userId
      name
      icon
      currency
      broker
      riskProfile
      createdAt
      updatedAt
      snaptradeUserId
      snaptradeConnectionId
      snaptradeConnectionDisabled
      snaptradeConnectionDisabledAt
    }
  }
`;

const DELETE_ACCOUNT_MUTATION = `
  mutation DeleteAccount($id: String!) {
    deleteAccount(id: $id)
  }
`;

// ---------------------------------------------------------------------------
// Service functions
// ---------------------------------------------------------------------------

export async function fetchAccounts(fetcher: GraphQLFetcher): Promise<Account[]> {
  const data = await fetcher<{ accounts: Account[] }>(ACCOUNTS_QUERY);
  return data.accounts;
}

export async function fetchAccount(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<Account | null> {
  const data = await fetcher<{ account: Account | null }>(ACCOUNT_QUERY, { id });
  return data.account;
}

export async function createAccount(
  fetcher: GraphQLFetcher,
  input: CreateAccountInput,
): Promise<Account> {
  const data = await fetcher<{ createAccount: Account }>(CREATE_ACCOUNT_MUTATION, { input });
  return data.createAccount;
}

export async function updateAccount(
  fetcher: GraphQLFetcher,
  id: string,
  input: UpdateAccountInput,
): Promise<Account> {
  const data = await fetcher<{ updateAccount: Account }>(UPDATE_ACCOUNT_MUTATION, { id, input });
  return data.updateAccount;
}

export async function deleteAccount(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteAccount: boolean }>(DELETE_ACCOUNT_MUTATION, { id });
  return data.deleteAccount;
}
