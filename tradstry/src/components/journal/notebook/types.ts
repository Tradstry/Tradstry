// In-memory notebook model. Field names mirror the backend GraphQL shape
// (documentJson is Lexical editor-state JSON, exactly like the web) so wiring
// the real API later is a drop-in swap.

export type Folder = {
  id: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
  /** System-owned (the agent-written notes folder). Not renamable or deletable. */
  isSystem: boolean;
};

export type Note = {
  id: string;
  folderId: string | null;
  /** Server-derived from the document's first H1; never an input. */
  title: string;
  /** Lexical `SerializedEditorState` JSON string (same format as the web). */
  documentJson: string;
  tradeIds: string[];
  sortOrder: number;
};
