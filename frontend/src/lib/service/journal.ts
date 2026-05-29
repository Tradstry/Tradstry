import type { GraphQLFetcher } from "@/lib/client";
import type {
  CreateJournalEntryInput,
  JournalEntry,
  UpdateJournalEntryInput,
} from "@/lib/types/journal";

const JOURNAL_ENTRY_FIELDS = `
  id
  userId
  accountId
  reviewed
  openDate
  closeDate
  entryPrice
  exitPrice
  positionSize
  symbol
  symbolName
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
`;

const JOURNAL_ENTRIES_QUERY = `
  query JournalEntries {
    journalEntries {
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
): Promise<JournalEntry[]> {
  const data = await fetcher<{ journalEntries: JournalEntry[] }>(
    JOURNAL_ENTRIES_QUERY,
  );
  return data.journalEntries;
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
