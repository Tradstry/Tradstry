import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";

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
  query UserAgents($workspaceId: String!) {
    userAgents(workspaceId: $workspaceId) {
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
  workspaceId: string,
): Promise<UserAgent[]> {
  const data = await fetcher<{ userAgents: UserAgent[] }>(
    USER_AGENTS_QUERY,
    { workspaceId },
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
