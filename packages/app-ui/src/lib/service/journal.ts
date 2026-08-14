import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import type {
  CreateJournalEntryInput,
  JournalEntry,
  PublishBrokerageEpisodeReviewInput,
  UpdateJournalEntryInput,
} from "@tradstry/app-ui/lib/types/journal";

const JOURNAL_ENTRY_FIELDS = `
  id
  userId
  workspaceId
  openDate
  closeDate
  entryPrice
  exitPrice
  positionSize
  symbol
  symbolName
  contractMultiplier
  status
  totalPl
  netRoi
  duration
  stopLoss
  riskReward
  tradeType
  mistakes
  entryTactics
  edgesSpotted
  tags {
    id
    categoryId
    name
    color
  }
  playbookId
  notes
  createdAt
`;

const JOURNAL_ENTRIES_QUERY = `
  query JournalEntries($workspaceId: String, $limit: Int, $afterCreatedAt: String, $afterId: String) {
    journalEntries(workspaceId: $workspaceId, limit: $limit, afterCreatedAt: $afterCreatedAt, afterId: $afterId) {
      ${JOURNAL_ENTRY_FIELDS}
    }
  }
`;

const JOURNAL_ENTRY_QUERY = `
  query JournalEntry($id: String!) {
    journalEntry(id: $id) {
      ${JOURNAL_ENTRY_FIELDS}
    }
  }
`;

const CREATE_JOURNAL_ENTRY_MUTATION = `
  mutation CreateJournalEntry($input: CreateJournalEntryInput!) {
    createJournalEntry(input: $input) {
      ${JOURNAL_ENTRY_FIELDS}
    }
  }
`;

const PUBLISH_BROKERAGE_EPISODE_REVIEW_MUTATION = `
  mutation PublishBrokerageEpisodeReview($input: PublishBrokerageEpisodeReviewInput!) {
    publishBrokerageEpisodeReview(input: $input)
  }
`;

const UPDATE_JOURNAL_ENTRY_MUTATION = `
  mutation UpdateJournalEntry($id: String!, $input: UpdateJournalEntryInput!) {
    updateJournalEntry(id: $id, input: $input) {
      ${JOURNAL_ENTRY_FIELDS}
    }
  }
`;

const DELETE_JOURNAL_ENTRY_MUTATION = `
  mutation DeleteJournalEntry($id: String!) {
    deleteJournalEntry(id: $id)
  }
`;

export async function fetchJournalEntries(
  fetcher: GraphQLFetcher,
  workspaceId?: string,
): Promise<JournalEntry[]> {
  const entries: JournalEntry[] = [];
  let afterCreatedAt: string | undefined;
  let afterId: string | undefined;

  for (;;) {
    const data = await fetcher<{ journalEntries: JournalEntry[] }>(
      JOURNAL_ENTRIES_QUERY,
      { workspaceId, limit: 500, afterCreatedAt, afterId },
    );
    entries.push(...data.journalEntries);
    if (data.journalEntries.length < 500) {
      return entries;
    }
    const last = data.journalEntries.at(-1);
    afterCreatedAt = last?.createdAt;
    afterId = last?.id;
  }
}

export async function fetchJournalEntry(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<JournalEntry | null> {
  const data = await fetcher<{ journalEntry: JournalEntry | null }>(
    JOURNAL_ENTRY_QUERY,
    { id },
  );
  return data.journalEntry;
}

export async function createJournalEntry(
  fetcher: GraphQLFetcher,
  input: CreateJournalEntryInput,
): Promise<JournalEntry> {
  const data = await fetcher<{ createJournalEntry: JournalEntry }>(
    CREATE_JOURNAL_ENTRY_MUTATION,
    { input },
  );
  return data.createJournalEntry;
}

export async function publishBrokerageEpisodeReview(
  fetcher: GraphQLFetcher,
  input: PublishBrokerageEpisodeReviewInput,
): Promise<string> {
  const data = await fetcher<{ publishBrokerageEpisodeReview: string }>(
    PUBLISH_BROKERAGE_EPISODE_REVIEW_MUTATION,
    { input },
  );
  return data.publishBrokerageEpisodeReview;
}

export async function updateJournalEntry(
  fetcher: GraphQLFetcher,
  id: string,
  input: UpdateJournalEntryInput,
): Promise<JournalEntry> {
  const data = await fetcher<{ updateJournalEntry: JournalEntry }>(
    UPDATE_JOURNAL_ENTRY_MUTATION,
    { id, input },
  );
  return data.updateJournalEntry;
}

export async function deleteJournalEntry(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteJournalEntry: boolean }>(
    DELETE_JOURNAL_ENTRY_MUTATION,
    { id },
  );
  return data.deleteJournalEntry;
}
