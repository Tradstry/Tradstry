import type { GraphQLFetcher } from "@/lib/client";
import type {
  CreateWorkspaceInput,
  UpdateWorkspaceInput,
  Workspace,
} from "@/lib/types/workspaces";

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

const WORKSPACE_FIELDS = `
  id
  userId
  name
  icon
  currency
  assetClass
  broker
  riskProfile
  totalValue
  totalValueCurrency
  createdAt
  updatedAt
  snaptradeUserId
  snaptradeConnectionId
  snaptradeAccountId
  snaptradeConnectionDisabled
  snaptradeConnectionDisabledAt
`;

const WORKSPACES_QUERY = `
  query Workspaces {
    workspaces {
      ${WORKSPACE_FIELDS}
    }
  }
`;

const WORKSPACE_QUERY = `
  query Workspace($id: String!) {
    workspace(id: $id) {
      ${WORKSPACE_FIELDS}
    }
  }
`;

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

const CREATE_WORKSPACE_MUTATION = `
  mutation CreateWorkspace($input: CreateWorkspaceInput!) {
    createWorkspace(input: $input) {
      ${WORKSPACE_FIELDS}
    }
  }
`;

const UPDATE_WORKSPACE_MUTATION = `
  mutation UpdateWorkspace($id: String!, $input: UpdateWorkspaceInput!) {
    updateWorkspace(id: $id, input: $input) {
      ${WORKSPACE_FIELDS}
    }
  }
`;

const DELETE_WORKSPACE_MUTATION = `
  mutation DeleteWorkspace($id: String!) {
    deleteWorkspace(id: $id)
  }
`;

// ---------------------------------------------------------------------------
// Service functions
// ---------------------------------------------------------------------------

export async function fetchWorkspaces(
  fetcher: GraphQLFetcher,
): Promise<Workspace[]> {
  const data = await fetcher<{ workspaces: Workspace[] }>(WORKSPACES_QUERY);
  return data.workspaces;
}

export async function fetchWorkspace(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<Workspace | null> {
  const data = await fetcher<{ workspace: Workspace | null }>(WORKSPACE_QUERY, {
    id,
  });
  return data.workspace;
}

export async function createWorkspace(
  fetcher: GraphQLFetcher,
  input: CreateWorkspaceInput,
): Promise<Workspace> {
  const data = await fetcher<{ createWorkspace: Workspace }>(
    CREATE_WORKSPACE_MUTATION,
    { input },
  );
  return data.createWorkspace;
}

export async function updateWorkspace(
  fetcher: GraphQLFetcher,
  id: string,
  input: UpdateWorkspaceInput,
): Promise<Workspace> {
  const data = await fetcher<{ updateWorkspace: Workspace }>(
    UPDATE_WORKSPACE_MUTATION,
    { id, input },
  );
  return data.updateWorkspace;
}

export async function deleteWorkspace(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteWorkspace: boolean }>(
    DELETE_WORKSPACE_MUTATION,
    { id },
  );
  return data.deleteWorkspace;
}
