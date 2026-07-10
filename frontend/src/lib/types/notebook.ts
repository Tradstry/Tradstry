export interface NotebookImage {
  id: string;
  noteId: string;
  userId: string;
  accountId: string;
  cloudinaryAssetId: string;
  cloudinaryPublicId: string;
  secureUrl: string;
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
  accountId: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface NotebookNote {
  id: string;
  userId: string;
  accountId: string;
  title: string;
  documentJson: string;
  tradeIds: string[];
  images: NotebookImage[];
  folderId: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface CreateNotebookNoteInput {
  accountId: string;
  documentJson: string;
  tradeIds?: string[];
  folderId?: string | null;
}

export interface UpdateNotebookNoteInput {
  accountId?: string;
  documentJson?: string;
  tradeIds?: string[];
  folderId?: string | null;
  /** Caller's last-known updatedAt; the server rejects the write as stale if
   * the row has since moved on. */
  expectedUpdatedAt?: string;
}

export interface CreateNotebookFolderInput {
  accountId: string;
  parentFolderId: string | null;
  name: string;
}

export interface MoveNotebookNodeInput {
  accountId: string;
  nodeId: string;
  nodeType: NotebookNodeType;
  newParentFolderId: string | null;
  newSortOrder: number;
}
