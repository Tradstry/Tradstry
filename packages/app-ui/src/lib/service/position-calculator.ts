import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import type {
	CreatePositionCalculatorHistoryInput,
	CreatePositionCalculatorPlanInput,
	PositionCalculatorHistoryEntry,
	PositionCalculatorPlan,
	PositionCalculatorRule,
	TradeReviewInboxItem,
	UpdatePositionCalculatorPlanInput,
	UpsertPositionCalculatorRuleInput,
} from "@tradstry/app-ui/lib/types/position-calculator";

const TRADE_REVIEW_FIELDS = `
  episodeId
  instrumentKey
  direction
  openedAt
  closedAt
  currentQuantity
  status
  blockReason
  matchStatus
  confirmedMatchId
  confirmedPlanId
  suggestionsJson
  latestReviewJson
`;

const TRADE_REVIEW_INBOX_QUERY = `
  query TradeReviewInbox($workspaceId: String!) {
    tradeReviewInbox(workspaceId: $workspaceId) { ${TRADE_REVIEW_FIELDS} }
  }
`;

const REQUEST_EXECUTION_CHECK_MUTATION = `
  mutation RequestPlanExecutionCheck($planId: String!) {
    requestPlanExecutionCheck(planId: $planId)
  }
`;

const CONFIRM_TRADE_MATCH_MUTATION = `
  mutation ConfirmTradeEpisodeMatch($episodeId: String!, $planId: String!) {
    confirmTradeEpisodeMatch(episodeId: $episodeId, planId: $planId)
  }
`;

const FINALIZE_TRADE_REVIEW_MUTATION = `
  mutation FinalizeTradeReview($matchId: String!, $reflectionJson: String!, $journalDraftJson: String, $noAdditionalContext: Boolean!) {
    finalizeTradeReview(matchId: $matchId, reflectionJson: $reflectionJson, journalDraftJson: $journalDraftJson, noAdditionalContext: $noAdditionalContext)
  }
`;

const PUBLISH_TRADE_REVIEW_MUTATION = `
  mutation PublishTradeReview($matchId: String!) {
    publishTradeReview(matchId: $matchId)
  }
`;

export async function fetchTradeReviewInbox(
	fetcher: GraphQLFetcher,
	workspaceId: string,
): Promise<TradeReviewInboxItem[]> {
	const data = await fetcher<{ tradeReviewInbox: TradeReviewInboxItem[] }>(
		TRADE_REVIEW_INBOX_QUERY,
		{ workspaceId },
	);
	return data.tradeReviewInbox;
}

export async function requestExecutionCheck(fetcher: GraphQLFetcher, planId: string) {
	const data = await fetcher<{ requestPlanExecutionCheck: number }>(REQUEST_EXECUTION_CHECK_MUTATION, { planId });
	return data.requestPlanExecutionCheck;
}

export async function confirmTradeMatch(fetcher: GraphQLFetcher, episodeId: string, planId: string) {
	const data = await fetcher<{ confirmTradeEpisodeMatch: string }>(CONFIRM_TRADE_MATCH_MUTATION, { episodeId, planId });
	return data.confirmTradeEpisodeMatch;
}

export async function finalizeTradeReview(
	fetcher: GraphQLFetcher,
	input: { matchId: string; reflectionJson: string; journalDraftJson?: string | null; noAdditionalContext: boolean },
) {
	const data = await fetcher<{ finalizeTradeReview: string }>(FINALIZE_TRADE_REVIEW_MUTATION, input);
	return data.finalizeTradeReview;
}

export async function publishTradeReview(fetcher: GraphQLFetcher, matchId: string) {
	const data = await fetcher<{ publishTradeReview: string }>(PUBLISH_TRADE_REVIEW_MUTATION, { matchId });
	return data.publishTradeReview;
}

const RULE_FIELDS = `
  id
  userId
  workspaceId
  workspaceId
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
  planId
  tranches {
    id
    percent
    shares
    targetPrice
    status
    filledAt
  }
  createdAt
`;

const POSITION_CALCULATOR_RULE_QUERY = `
  query PositionCalculatorRule($workspaceId: String!) {
    positionCalculatorRule(workspaceId: $workspaceId) {
      ${RULE_FIELDS}
    }
  }
`;

const POSITION_CALCULATOR_HISTORY_QUERY = `
  query PositionCalculatorHistory($workspaceId: String!) {
    positionCalculatorHistory(workspaceId: $workspaceId) {
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
	workspaceId: string,
): Promise<PositionCalculatorRule | null> {
	const data = await fetcher<{
		positionCalculatorRule: PositionCalculatorRule | null;
	}>(POSITION_CALCULATOR_RULE_QUERY, { workspaceId });
	return data.positionCalculatorRule;
}

export async function fetchHistory(
	fetcher: GraphQLFetcher,
	workspaceId: string,
): Promise<PositionCalculatorHistoryEntry[]> {
	const data = await fetcher<{
		positionCalculatorHistory: PositionCalculatorHistoryEntry[];
	}>(POSITION_CALCULATOR_HISTORY_QUERY, { workspaceId });
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
  workspaceId
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
  instrumentJson
  createdAt
  updatedAt
`;

const PLANS_QUERY = `
  query PositionCalculatorPlans($workspaceId: String!) {
    positionCalculatorPlans(workspaceId: $workspaceId) {
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
	workspaceId: string,
): Promise<PositionCalculatorPlan[]> {
	const data = await fetcher<{
		positionCalculatorPlans: PositionCalculatorPlan[];
	}>(PLANS_QUERY, { workspaceId });
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
