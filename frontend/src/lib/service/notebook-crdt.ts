import type { GraphQLFetcher } from "@/lib/client";

const NOTEBOOK_UPDATES_SINCE_QUERY = `
  query NotebookUpdatesSince($noteId: String!, $sinceSeq: Int!) {
    notebookUpdatesSince(noteId: $noteId, sinceSeq: $sinceSeq) {
      seq
      update
    }
  }
`;

const APPEND_NOTEBOOK_UPDATES_MUTATION = `
  mutation AppendNotebookUpdates($noteId: String!, $updates: [String!]!) {
    appendNotebookUpdates(noteId: $noteId, updates: $updates)
  }
`;

/** A Yjs update blob, base64 of the raw bytes, and its global append sequence. */
export type NotebookUpdate = {
  seq: number;
  update: string;
};

export async function fetchNotebookUpdates(
  fetcher: GraphQLFetcher,
  noteId: string,
  sinceSeq: number,
): Promise<NotebookUpdate[]> {
  const data = await fetcher<{ notebookUpdatesSince: NotebookUpdate[] }>(
    NOTEBOOK_UPDATES_SINCE_QUERY,
    { noteId, sinceSeq },
  );
  return data.notebookUpdatesSince;
}

/** Returns the note's new max `seq`. */
export async function appendNotebookUpdates(
  fetcher: GraphQLFetcher,
  noteId: string,
  updates: string[],
): Promise<number> {
  const data = await fetcher<{ appendNotebookUpdates: number }>(
    APPEND_NOTEBOOK_UPDATES_MUTATION,
    { noteId, updates },
  );
  return data.appendNotebookUpdates;
}
