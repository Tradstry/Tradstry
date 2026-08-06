export interface NotebookImage {
  id: string;
  noteId: string;
  userId: string;
  workspaceId: string;
  cloudinaryAssetId: string;
  cloudinaryPublicId: string;
  secureUrl: string;
  contentHash: string;
  width: number;
  height: number;
  format: string;
  bytes: number;
  originalFilename: string;
  mediaType: string;
  contentType: string;
  durationSeconds: number;
  createdAt: string;
}

export type NotebookNodeType = "FOLDER" | "NOTE";

export interface NotebookFolder {
  id: string;
  userId: string;
  workspaceId: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
  /** System-owned (the agent-written notes folder). Not renamable or deletable. */
  isSystem: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface NotebookNote {
  id: string;
  userId: string;
  workspaceId: string;
  title: string;
  documentJson: string;
  tradeIds: string[];
  images: NotebookImage[];
  folderId: string | null;
  sortOrder: number;
  isStarred: boolean;
  isPinned: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface CreateNotebookNoteInput {
  workspaceId: string;
  documentJson: string;
  tradeIds?: string[];
  folderId?: string | null;
}

export interface UpdateNotebookNoteInput {
  workspaceId?: string;
  documentJson?: string;
  tradeIds?: string[];
  folderId?: string | null;
  /** Caller's last-known updatedAt; the server rejects the write as stale if
   * the row has since moved on. */
  expectedUpdatedAt?: string;
}

export interface CreateNotebookFolderInput {
  workspaceId: string;
  parentFolderId: string | null;
  name: string;
}

export interface MoveNotebookNodeInput {
  workspaceId: string;
  nodeId: string;
  nodeType: NotebookNodeType;
  newParentFolderId: string | null;
  newSortOrder: number;
}
