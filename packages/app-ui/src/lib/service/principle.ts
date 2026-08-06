import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import type {
  CreatePrincipleInput,
  PrincipleWithStats,
  UpdatePrincipleInput,
} from "@tradstry/app-ui/lib/types/principle";

const PRINCIPLE_FIELDS = `
  id
  userId
  workspaceId
  playbookId
  evidenceNoteId
  evidenceNoteTitle
  title
  theRule
  why
  intervention
  priority
  isActive
  createdAt
  updatedAt
  violationCount
  violatedCumulativeProfit
  violatedCumulativeRoi
  violatedWinRate
`;

const PRINCIPLES_QUERY = `
  query Principles($workspaceId: String!) {
    principles(workspaceId: $workspaceId) {
      ${PRINCIPLE_FIELDS}
    }
  }
`;

const CREATE_PRINCIPLE_MUTATION = `
  mutation CreatePrinciple($input: CreatePrincipleInput!) {
    createPrinciple(input: $input) {
      ${PRINCIPLE_FIELDS}
    }
  }
`;

const UPDATE_PRINCIPLE_MUTATION = `
  mutation UpdatePrinciple($id: String!, $input: UpdatePrincipleInput!) {
    updatePrinciple(id: $id, input: $input) {
      ${PRINCIPLE_FIELDS}
    }
  }
`;

const DELETE_PRINCIPLE_MUTATION = `
  mutation DeletePrinciple($id: String!) {
    deletePrinciple(id: $id)
  }
`;

const REORDER_PRINCIPLES_MUTATION = `
  mutation ReorderPrinciples($orderedIds: [String!]!) {
    reorderPrinciples(orderedIds: $orderedIds)
  }
`;

const TRADE_VIOLATIONS_QUERY = `
  query TradeViolatedPrincipleIds($journalEntryId: String!) {
    tradeViolatedPrincipleIds(journalEntryId: $journalEntryId)
  }
`;

export async function fetchPrinciples(
  fetcher: GraphQLFetcher,
  workspaceId: string,
): Promise<PrincipleWithStats[]> {
  const data = await fetcher<{ principles: PrincipleWithStats[] }>(
    PRINCIPLES_QUERY,
    { workspaceId },
  );
  return data.principles;
}

export async function createPrinciple(
  fetcher: GraphQLFetcher,
  input: CreatePrincipleInput,
): Promise<PrincipleWithStats> {
  const data = await fetcher<{ createPrinciple: PrincipleWithStats }>(
    CREATE_PRINCIPLE_MUTATION,
    { input },
  );
  return data.createPrinciple;
}

export async function updatePrinciple(
  fetcher: GraphQLFetcher,
  id: string,
  input: UpdatePrincipleInput,
): Promise<PrincipleWithStats> {
  const data = await fetcher<{ updatePrinciple: PrincipleWithStats }>(
    UPDATE_PRINCIPLE_MUTATION,
    { id, input },
  );
  return data.updatePrinciple;
}

export async function deletePrinciple(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deletePrinciple: boolean }>(
    DELETE_PRINCIPLE_MUTATION,
    { id },
  );
  return data.deletePrinciple;
}

export async function reorderPrinciples(
  fetcher: GraphQLFetcher,
  orderedIds: string[],
): Promise<boolean> {
  const data = await fetcher<{ reorderPrinciples: boolean }>(
    REORDER_PRINCIPLES_MUTATION,
    { orderedIds },
  );
  return data.reorderPrinciples;
}

export async function fetchTradeViolatedPrincipleIds(
  fetcher: GraphQLFetcher,
  journalEntryId: string,
): Promise<string[]> {
  const data = await fetcher<{ tradeViolatedPrincipleIds: string[] }>(
    TRADE_VIOLATIONS_QUERY,
    { journalEntryId },
  );
  return data.tradeViolatedPrincipleIds;
}
