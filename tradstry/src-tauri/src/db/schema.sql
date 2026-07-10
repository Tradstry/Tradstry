-- Local mirror of the server notebook, plus the transactional outbox.
-- Rows are never hard-deleted; `deleted_at` is a tombstone, matching the server.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS folders (
    id                 TEXT PRIMARY KEY NOT NULL,
    account_id         TEXT NOT NULL,
    parent_folder_id   TEXT NULL,
    name               TEXT NOT NULL,
    sort_order         INTEGER NOT NULL DEFAULT 0,
    hlc_name           TEXT NOT NULL DEFAULT '',
    hlc_parent         TEXT NOT NULL DEFAULT '',
    hlc_sort_order     TEXT NOT NULL DEFAULT '',
    deleted_at         TEXT NULL,
    sync_state         TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_folders_account ON folders (account_id);

CREATE TABLE IF NOT EXISTS notes (
    id                 TEXT PRIMARY KEY NOT NULL,
    account_id         TEXT NOT NULL,
    folder_id          TEXT NULL,
    -- Cache of the server-derived title (first H1 of the document). Never an
    -- input, never stamped: it is a pure function of document_json.
    title              TEXT NOT NULL DEFAULT 'Untitled',
    document_json      TEXT NOT NULL,
    sort_order         INTEGER NOT NULL DEFAULT 0,
    trade_ids          TEXT NOT NULL DEFAULT '[]',  -- JSON array; a set, not a list
    hlc_folder_id      TEXT NOT NULL DEFAULT '',
    hlc_sort_order     TEXT NOT NULL DEFAULT '',
    hlc_trade_ids      TEXT NOT NULL DEFAULT '',
    body_hlc           TEXT NOT NULL DEFAULT '',
    deleted_at         TEXT NULL,
    sync_state         TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_notes_account ON notes (account_id);
CREATE INDEX IF NOT EXISTS idx_notes_live ON notes (account_id) WHERE deleted_at IS NULL;

-- Transactional outbox. Written in the SAME transaction as the data row, so a
-- crash can never leave a write that will not be synced, or vice versa.
CREATE TABLE IF NOT EXISTS outbox (
    mutation_id INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    args        TEXT NOT NULL,  -- JSON. Contains every generated id and stamp.
    hlc         TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Yjs CRDT update blobs for note bodies. Rust never interprets these: they are
-- an ordered, append-only byte pipe. `update` is a BLOB and must stay one.
CREATE TABLE IF NOT EXISTS note_updates (
    note_id TEXT NOT NULL,
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    "update" BLOB NOT NULL,
    synced  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_note_updates_note ON note_updates (note_id, seq);

CREATE TABLE IF NOT EXISTS sync_meta (
    account_id       TEXT PRIMARY KEY NOT NULL,
    cookie           TEXT NULL,
    last_mutation_id INTEGER NOT NULL DEFAULT 0,
    last_sync_at     TEXT NULL
);

-- One row, created on first open.
CREATE TABLE IF NOT EXISTS client (
    id TEXT PRIMARY KEY NOT NULL
);

-- How far this device has consumed the server's global update sequence. Its own
-- table rather than a column on `sync_meta`: the schema is applied with
-- CREATE IF NOT EXISTS and has no migration step, so a new column would never
-- appear on an existing database.
CREATE TABLE IF NOT EXISTS update_cursor (
    account_id TEXT PRIMARY KEY NOT NULL,
    last_seq   INTEGER NOT NULL DEFAULT 0
);

-- Server update sequences this device has already stored. `seq` is the server's
-- global BIGSERIAL, so it is unique across notes and makes a pull idempotent even
-- when the cursor deliberately re-reads a window behind itself.
CREATE TABLE IF NOT EXISTS pulled_seq (
    seq INTEGER PRIMARY KEY NOT NULL
);
