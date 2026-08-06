import type { DesktopDatabase } from "./database.ts";
import { transaction } from "./database.ts";
import { decodeBase64Strict, enqueueMutation, uuidV7 } from "./mutations.ts";

export type NotebookNote = {
  id: string;
  folderId: string | null;
  title: string;
  documentJson: string;
  tradeIds: string[];
  sortOrder: number;
};

export type NotebookFolder = {
  id: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
  isSystem: boolean;
};

type StoredNote = {
  id: string;
  folder_id: string | null;
  title: string;
  document_json: string;
  trade_ids: string;
  sort_order: number;
};

export class NotebookRepository {
  readonly #store: DesktopDatabase;

  constructor(store: DesktopDatabase) {
    this.#store = store;
  }

  notes(accountId: string): NotebookNote[] {
    const rows = this.#store.db
      .prepare(
        `SELECT id, folder_id, title, document_json, trade_ids, sort_order
         FROM notes WHERE account_id = ? AND deleted_at IS NULL
         ORDER BY sort_order ASC, id ASC`,
      )
      .all(accountId) as StoredNote[];
    return rows.map((row) => ({
      id: row.id,
      folderId: row.folder_id,
      title: row.title,
      documentJson: row.document_json,
      tradeIds: stringArray(row.trade_ids),
      sortOrder: row.sort_order,
    }));
  }

  folders(accountId: string): NotebookFolder[] {
    return (this.#store.db
      .prepare(
        `SELECT id, parent_folder_id AS parentFolderId, name, sort_order AS sortOrder,
                is_system AS isSystem
         FROM folders WHERE account_id = ? AND deleted_at IS NULL
         ORDER BY sort_order ASC, name ASC`,
      )
      .all(accountId) as Array<Omit<NotebookFolder, "isSystem"> & { isSystem: number }>).map((row) => ({
        ...row,
        isSystem: Boolean(row.isSystem),
      }));
  }

  createNote(input: {
    accountId: string;
    folderId: string | null;
    documentJson: string;
    seedUpdateB64: string;
    seedStateVectorB64: string;
  }): string {
    const seed = decodeBase64Strict(input.seedUpdateB64);
    const stateVector = decodeBase64Strict(input.seedStateVectorB64);
    const id = uuidV7();
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          "INSERT INTO notes (id, account_id, folder_id, title, document_json, body_hlc) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .run(id, input.accountId, input.folderId, deriveNoteTitle(input.documentJson), input.documentJson, stamp);
      this.#store.db.prepare('INSERT INTO note_updates (note_id, "update") VALUES (?, ?)').run(id, seed);
      enqueueMutation(this.#store.db, "createNote", {
        id,
        accountId: input.accountId,
        documentJson: input.documentJson,
        tradeIds: [],
        folderId: input.folderId,
        seedUpdate: Buffer.from(seed).toString("base64"),
        seedStateVector: Buffer.from(stateVector).toString("base64"),
      }, stamp);
    });
    return id;
  }

  cacheNoteBody(id: string, documentJson: string): void {
    this.#store.db
      .prepare("UPDATE notes SET document_json = ?, title = ? WHERE id = ?")
      .run(documentJson, deriveNoteTitle(documentJson), id);
  }

  appendNoteUpdate(noteId: string, updateB64: string): void {
    const update = decodeBase64Strict(updateB64);
    transaction(this.#store.db, () => {
      this.#store.db.prepare('INSERT INTO note_updates (note_id, "update") VALUES (?, ?)').run(noteId, update);
      enqueueMutation(this.#store.db, "appendNoteUpdate", { noteId, update: Buffer.from(update).toString("base64") }, "");
    });
  }

  noteUpdates(noteId: string): string[] {
    const rows = this.#store.db
      .prepare('SELECT "update" AS update_blob FROM note_updates WHERE note_id = ? ORDER BY seq ASC')
      .all(noteId) as Array<{ update_blob: Uint8Array }>;
    return rows.map((row) => Buffer.from(row.update_blob).toString("base64"));
  }

  moveNote(id: string, folderId: string | null, sortOrder: number): void {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      const row = this.#store.db.prepare("SELECT account_id FROM notes WHERE id = ?").get(id) as
        | { account_id: string }
        | undefined;
      if (!row) throw new Error(`note not found: ${id}`);
      this.#store.db
        .prepare(
          `UPDATE notes SET folder_id = ?, sort_order = ?, hlc_folder_id = ?,
                            hlc_sort_order = ?, sync_state = 'pending' WHERE id = ?`,
        )
        .run(folderId, sortOrder, stamp, stamp, id);
      enqueueMutation(this.#store.db, "moveNode", {
        accountId: row.account_id,
        nodeId: id,
        nodeType: "note",
        newParentFolderId: folderId,
        newSortOrder: sortOrder,
      }, stamp);
    });
  }

  deleteNote(id: string): void {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("UPDATE notes SET deleted_at = datetime('now'), sync_state = 'pending' WHERE id = ?")
        .run(id);
      enqueueMutation(this.#store.db, "deleteNote", { id }, stamp);
    });
  }

  createFolder(accountId: string, name: string): string {
    const id = uuidV7();
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          `INSERT INTO folders
           (id, account_id, parent_folder_id, name, sort_order, hlc_name, hlc_parent, hlc_sort_order, sync_state)
           VALUES (?, ?, NULL, ?, 0, ?, ?, ?, 'pending')`,
        )
        .run(id, accountId, name, stamp, stamp, stamp);
      enqueueMutation(this.#store.db, "createFolder", {
        id,
        accountId,
        name,
        parentFolderId: null,
        sortOrder: 0,
      }, stamp);
    });
    return id;
  }

  renameFolder(id: string, name: string): void {
    this.#rejectSystemFolder(id);
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("UPDATE folders SET name = ?, hlc_name = ?, sync_state = 'pending' WHERE id = ?")
        .run(name, stamp, id);
      enqueueMutation(this.#store.db, "renameFolder", { id, name }, stamp);
    });
  }

  deleteFolder(id: string): void {
    this.#rejectSystemFolder(id);
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      const subtree = `WITH RECURSIVE subtree(id) AS (
        SELECT id FROM folders WHERE id = ?
        UNION SELECT f.id FROM folders f JOIN subtree s ON f.parent_folder_id = s.id
      )`;
      this.#store.db
        .prepare(`${subtree} UPDATE folders SET deleted_at = datetime('now'), sync_state = 'pending' WHERE id IN (SELECT id FROM subtree)`)
        .run(id);
      this.#store.db
        .prepare(`${subtree} UPDATE notes SET deleted_at = datetime('now'), sync_state = 'pending' WHERE folder_id IN (SELECT id FROM subtree)`)
        .run(id);
      enqueueMutation(this.#store.db, "deleteFolder", { id }, stamp);
    });
  }

  #rejectSystemFolder(id: string): void {
    const row = this.#store.db.prepare("SELECT COALESCE(is_system, 0) AS is_system FROM folders WHERE id = ?").get(id) as
      | { is_system: number }
      | undefined;
    if (row?.is_system) throw new Error("The System folder cannot be renamed or deleted");
  }
}

export function deriveNoteTitle(documentJson: string): string {
  let document: unknown;
  try {
    document = JSON.parse(documentJson);
  } catch {
    return "Untitled";
  }
  const children = getChildren((document as { root?: unknown }).root);
  if (!children) return "Untitled";
  for (const child of children) {
    const node = child as { type?: unknown; tag?: unknown };
    if (node.type === "heading" && node.tag === "h1") {
      const title = nodeText(child);
      if (title) return title;
    }
  }
  for (const child of children) {
    const title = nodeText(child);
    if (title) return title;
  }
  return "Untitled";
}

function nodeText(node: unknown): string | null {
  if (!node || typeof node !== "object") return null;
  let result = typeof (node as { text?: unknown }).text === "string" ? (node as { text: string }).text : "";
  for (const child of getChildren(node) ?? []) result += nodeText(child) ?? "";
  return result.trim() || null;
}

function getChildren(node: unknown): unknown[] | null {
  if (!node || typeof node !== "object") return null;
  const children = (node as { children?: unknown }).children;
  return Array.isArray(children) ? children : null;
}

function stringArray(value: string): string[] {
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string") : [];
  } catch {
    return [];
  }
}
