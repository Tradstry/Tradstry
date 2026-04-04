import type { GraphQLFetcher } from "@/lib/client";

export interface UserAgent {
  id: string;
  name: string;
  goal: string;
  outputStyle: string;
  createdAt: string;
  updatedAt: string;
}

const AGENT_FIELDS = `
  id
  name
  goal
  outputStyle
  createdAt
  updatedAt
`;

const USER_AGENTS_QUERY = `
  query UserAgents($accountId: String!) {
    userAgents(accountId: $accountId) {
      ${AGENT_FIELDS}
    }
  }
`;

const DELETE_AGENT_MUTATION = `
  mutation DeleteUserAgent($id: String!) {
    deleteUserAgent(id: $id)
  }
`;

export async function fetchUserAgents(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<UserAgent[]> {
  const data = await fetcher<{ userAgents: UserAgent[] }>(
    USER_AGENTS_QUERY,
    { accountId },
  );
  return data.userAgents;
}

export async function deleteUserAgent(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteUserAgent: boolean }>(
    DELETE_AGENT_MUTATION,
    { id },
  );
  return data.deleteUserAgent;
}
