export type NoteRow = {
  id: string;
  folderId: string | null;
  title: string;
  documentJson: string;
  sortOrder: number;
  tradeIds: string[];
  hlcFolderId: string;
  hlcSortOrder: string;
  hlcTradeIds: string;
  bodyHlc: string;
  deletedAt: string | null;
};

export type MergeResult =
  | { kind: "unchanged" }
  | { kind: "take"; note: NoteRow }
  | { kind: "tombstone" };

function sameSet(left: string[], right: string[]): boolean {
  return [...left].sort().join("\0") === [...right].sort().join("\0");
}

/** Per-field LWW metadata merge. The Yjs body never travels through this path. */
export function mergeNote(local: NoteRow, server: NoteRow): MergeResult {
  if (server.deletedAt !== null || local.deletedAt !== null) return { kind: "tombstone" };

  const note: NoteRow = { ...server, tradeIds: [...server.tradeIds] };
  if (local.hlcFolderId > server.hlcFolderId) {
    note.folderId = local.folderId;
    note.hlcFolderId = local.hlcFolderId;
  }
  if (local.hlcSortOrder > server.hlcSortOrder) {
    note.sortOrder = local.sortOrder;
    note.hlcSortOrder = local.hlcSortOrder;
  }
  if (local.hlcTradeIds > server.hlcTradeIds) {
    note.tradeIds = [...local.tradeIds];
    note.hlcTradeIds = local.hlcTradeIds;
  }

  note.documentJson = local.documentJson;
  note.title = local.title;
  note.bodyHlc = local.bodyHlc;

  if (
    note.folderId === local.folderId &&
    note.sortOrder === local.sortOrder &&
    sameSet(note.tradeIds, local.tradeIds)
  ) {
    return { kind: "unchanged" };
  }
  return { kind: "take", note };
}
