import type { GraphQLFetcher } from "@/lib/client";
import type {
  CreatePositionCalculatorHistoryInput,
  CreatePositionCalculatorPlanInput,
  PositionCalculatorHistoryEntry,
  PositionCalculatorPlan,
  PositionCalculatorRule,
  UpdatePositionCalculatorPlanInput,
  UpsertPositionCalculatorRuleInput,
} from "@/lib/types/position-calculator";

const RULE_FIELDS = `
  id
  userId
  accountId
  accountBalance
  accountRisk
  maxStopLossPct
  createdAt
  updatedAt
`;

const HISTORY_FIELDS = `
  id
  userId
  symbol
  positionType
  entryPrice
  stopLoss
  accountBalance
  accountRisk
  shares
  positionValue
  accountPct
  stopLossPct
  createdAt
`;

const POSITION_CALCULATOR_RULE_QUERY = `
  query PositionCalculatorRule($accountId: String!) {
    positionCalculatorRule(accountId: $accountId) {
      ${RULE_FIELDS}
    }
  }
`;

const POSITION_CALCULATOR_HISTORY_QUERY = `
  query PositionCalculatorHistory {
    positionCalculatorHistory {
      ${HISTORY_FIELDS}
    }
  }
`;

const UPSERT_RULE_MUTATION = `
  mutation UpsertPositionCalculatorRule($input: UpsertPositionCalculatorRuleInput!) {
    upsertPositionCalculatorRule(input: $input) {
      ${RULE_FIELDS}
    }
  }
`;

const CREATE_HISTORY_MUTATION = `
  mutation CreatePositionCalculatorHistory($input: CreatePositionCalculatorHistoryInput!) {
    createPositionCalculatorHistory(input: $input) {
      ${HISTORY_FIELDS}
    }
  }
`;

const DELETE_HISTORY_MUTATION = `
  mutation DeletePositionCalculatorHistory($id: String!) {
    deletePositionCalculatorHistory(id: $id)
  }
`;

export async function fetchRule(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<PositionCalculatorRule | null> {
  const data = await fetcher<{
    positionCalculatorRule: PositionCalculatorRule | null;
  }>(POSITION_CALCULATOR_RULE_QUERY, { accountId });
  return data.positionCalculatorRule;
}

export async function fetchHistory(
  fetcher: GraphQLFetcher,
): Promise<PositionCalculatorHistoryEntry[]> {
  const data = await fetcher<{
    positionCalculatorHistory: PositionCalculatorHistoryEntry[];
  }>(POSITION_CALCULATOR_HISTORY_QUERY);
  return data.positionCalculatorHistory;
}

export async function upsertRule(
  fetcher: GraphQLFetcher,
  input: UpsertPositionCalculatorRuleInput,
): Promise<PositionCalculatorRule> {
  const data = await fetcher<{
    upsertPositionCalculatorRule: PositionCalculatorRule;
  }>(UPSERT_RULE_MUTATION, { input });
  return data.upsertPositionCalculatorRule;
}

export async function createHistoryEntry(
  fetcher: GraphQLFetcher,
  input: CreatePositionCalculatorHistoryInput,
): Promise<PositionCalculatorHistoryEntry> {
  const data = await fetcher<{
    createPositionCalculatorHistory: PositionCalculatorHistoryEntry;
  }>(CREATE_HISTORY_MUTATION, { input });
  return data.createPositionCalculatorHistory;
}

export async function deleteHistoryEntry(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deletePositionCalculatorHistory: boolean }>(
    DELETE_HISTORY_MUTATION,
    { id },
  );
  return data.deletePositionCalculatorHistory;
}

// ---------------------------------------------------------------------------
// Plans
// ---------------------------------------------------------------------------

const TRANCHE_FIELDS = `
  id
  percent
  shares
  targetPrice
  status
  filledAt
`;

const PLAN_FIELDS = `
  id
  userId
  symbol
  positionType
  entryPrice
  stopLoss
  accountBalance
  accountRisk
  totalShares
  positionValue
  status
  tranches {
    ${TRANCHE_FIELDS}
  }
  notes
  createdAt
  updatedAt
`;

const PLANS_QUERY = `
  query PositionCalculatorPlans {
    positionCalculatorPlans {
      ${PLAN_FIELDS}
    }
  }
`;

const CREATE_PLAN_MUTATION = `
  mutation CreatePositionCalculatorPlan($input: CreatePositionCalculatorPlanInput!) {
    createPositionCalculatorPlan(input: $input) {
      ${PLAN_FIELDS}
    }
  }
`;

const UPDATE_PLAN_MUTATION = `
  mutation UpdatePositionCalculatorPlan($id: String!, $input: UpdatePositionCalculatorPlanInput!) {
    updatePositionCalculatorPlan(id: $id, input: $input) {
      ${PLAN_FIELDS}
    }
  }
`;

const DELETE_PLAN_MUTATION = `
  mutation DeletePositionCalculatorPlan($id: String!) {
    deletePositionCalculatorPlan(id: $id)
  }
`;

export async function fetchPlans(
  fetcher: GraphQLFetcher,
): Promise<PositionCalculatorPlan[]> {
  const data = await fetcher<{
    positionCalculatorPlans: PositionCalculatorPlan[];
  }>(PLANS_QUERY);
  return data.positionCalculatorPlans;
}

export async function createPlan(
  fetcher: GraphQLFetcher,
  input: CreatePositionCalculatorPlanInput,
): Promise<PositionCalculatorPlan> {
  const data = await fetcher<{
    createPositionCalculatorPlan: PositionCalculatorPlan;
  }>(CREATE_PLAN_MUTATION, { input });
  return data.createPositionCalculatorPlan;
}

export async function updatePlan(
  fetcher: GraphQLFetcher,
  id: string,
  input: UpdatePositionCalculatorPlanInput,
): Promise<PositionCalculatorPlan> {
  const data = await fetcher<{
    updatePositionCalculatorPlan: PositionCalculatorPlan;
  }>(UPDATE_PLAN_MUTATION, { id, input });
  return data.updatePositionCalculatorPlan;
}

export async function deletePlan(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deletePositionCalculatorPlan: boolean }>(
    DELETE_PLAN_MUTATION,
    { id },
  );
  return data.deletePositionCalculatorPlan;
}
