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

-- Content-addressed media (images/video) referenced by notebook nodes via
-- `hash`. Bytes live on disk (`local_path`/`thumb_path`); this row tracks
-- where they are and whether the server has them yet.
CREATE TABLE IF NOT EXISTS notebook_media (
    hash             TEXT PRIMARY KEY,
    note_id          TEXT NOT NULL,
    account_id       TEXT NOT NULL,
    mime             TEXT NOT NULL,
    media_type       TEXT NOT NULL,
    width            INTEGER NOT NULL DEFAULT 0,
    height           INTEGER NOT NULL DEFAULT 0,
    duration_seconds REAL NOT NULL DEFAULT 0,
    bytes            INTEGER NOT NULL DEFAULT 0,
    original_filename TEXT NOT NULL DEFAULT '',
    local_path       TEXT,
    thumb_path       TEXT,
    upload_state     TEXT NOT NULL DEFAULT 'pending',
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_notebook_media_pending ON notebook_media (account_id, upload_state);

-- Playbooks are user-scoped (no account_id). Whole-row LWW via a single `hlc`;
-- soft-delete via `deleted_at`. Stats are NOT stored here — they are an online
-- overlay computed from trades.
CREATE TABLE IF NOT EXISTS playbooks (
    id                     TEXT PRIMARY KEY NOT NULL,
    name                   TEXT NOT NULL,
    edge_name              TEXT NOT NULL DEFAULT '',
    entry_rules            TEXT NOT NULL DEFAULT '',
    exit_rules             TEXT NOT NULL DEFAULT '',
    position_sizing_rules  TEXT NOT NULL DEFAULT '',
    additional_rules       TEXT NULL,
    hlc                    TEXT NOT NULL DEFAULT '',
    deleted_at             TEXT NULL,
    sync_state             TEXT NOT NULL DEFAULT 'pending'
);

-- Single-row cursor for the user-scoped playbook pull (distinct from the
-- account-scoped `sync_meta`).
CREATE TABLE IF NOT EXISTS playbook_sync (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    cookie       TEXT NULL,
    last_sync_at TEXT NULL
);

-- Local mirror of a server trade row. Whole-row LWW via a single `hlc`;
-- soft-delete via `deleted_at`. Derived metrics (status/total_pl/net_roi/
-- duration/risk_reward) are computed on-device (sync::derive) and stored so
-- the UI reads them like any other column.
CREATE TABLE IF NOT EXISTS journal_entries (
    id                       TEXT PRIMARY KEY NOT NULL,
    account_id               TEXT NOT NULL,
    open_date                TEXT NOT NULL,
    close_date               TEXT NOT NULL,
    entry_price              REAL NOT NULL,
    exit_price               REAL NOT NULL,
    position_size            REAL NOT NULL,
    stop_loss                REAL NULL,
    symbol                   TEXT NOT NULL,
    symbol_name              TEXT NOT NULL DEFAULT '',
    status                   TEXT NOT NULL DEFAULT 'profit',
    total_pl                 REAL NOT NULL DEFAULT 0,
    net_roi                  REAL NOT NULL DEFAULT 0,
    duration                 INTEGER NOT NULL DEFAULT 0,
    risk_reward              REAL NULL,
    trade_type               TEXT NOT NULL,
    playbook_id              TEXT NULL,
    notes                    TEXT NULL,
    broke_30min_rule         INTEGER NULL,
    pre_trade_conviction     INTEGER NULL,
    market_regime            TEXT NULL,
    is_planned_pre_market    INTEGER NULL,
    revenge_trade            INTEGER NULL,
    rule_adherence_score     INTEGER NULL,
    tag_ids                  TEXT NOT NULL DEFAULT '[]', -- JSON array
    violated_principle_ids   TEXT NOT NULL DEFAULT '[]', -- JSON array
    created_at               TEXT NOT NULL DEFAULT (datetime('now')),
    hlc                      TEXT NOT NULL DEFAULT '',
    deleted_at               TEXT NULL,
    sync_state               TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_journal_entries_account ON journal_entries (account_id, deleted_at);

-- Per-account cursor for the journal pull (independent of `sync_meta`, whose
-- cookie tracks the notebook timeline instead).
CREATE TABLE IF NOT EXISTS journal_sync (
    account_id   TEXT PRIMARY KEY NOT NULL,
    cookie       TEXT NULL,
    last_sync_at TEXT NULL
);

-- Tags/categories are user-scoped (no account_id), same shape as playbooks:
-- whole-row LWW via a single `hlc`, soft-delete via `deleted_at`. Local CRUD
-- writes these directly and enqueues the matching outbox mutation.
CREATE TABLE IF NOT EXISTS tags_cache (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    color       TEXT NULL,
    category_id TEXT NULL,
    hlc         TEXT NOT NULL DEFAULT '',
    deleted_at  TEXT NULL,
    sync_state  TEXT NOT NULL DEFAULT 'pending'
);

CREATE TABLE IF NOT EXISTS tag_categories_cache (
    id         TEXT PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL,
    role       TEXT NULL,
    color      TEXT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    hlc        TEXT NOT NULL DEFAULT '',
    deleted_at TEXT NULL,
    sync_state TEXT NOT NULL DEFAULT 'pending'
);

-- Single-row cursor for the user-scoped tags delta pull (distinct from the
-- account-scoped `sync_meta` and the `playbook_sync` cursor).
CREATE TABLE IF NOT EXISTS tag_sync (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    cookie       TEXT NULL,
    last_sync_at TEXT NULL
);

-- Pull-only cache of the user's accounts, so the account switcher and
-- analytics equity lookups work offline. Never written by a local mutation;
-- refreshed wholesale from the server `accounts` query.
CREATE TABLE IF NOT EXISTS accounts_cache (
    id           TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL,
    broker       TEXT NULL,
    currency     TEXT NULL,
    icon         TEXT NULL,
    total_value  REAL NULL,
    risk_profile TEXT NULL
);

-- Trading principles are account-scoped, same shape as journal_entries:
-- whole-row LWW via a single `hlc`, soft-delete via `deleted_at`. Violation
-- stats (violation_count/violated_cumulative_profit/violated_win_rate/etc) are
-- NOT stored here — they are derived on-device from `journal_entries` at read
-- time (see `journal::principle`).
CREATE TABLE IF NOT EXISTS trading_principles (
    id                 TEXT PRIMARY KEY NOT NULL,
    account_id         TEXT NOT NULL,
    playbook_id        TEXT NULL,
    evidence_note_id   TEXT NULL,
    title              TEXT NOT NULL,
    the_rule           TEXT NOT NULL DEFAULT '',
    why                TEXT NOT NULL DEFAULT '',
    intervention       TEXT NULL,
    priority           INTEGER NOT NULL DEFAULT 0,
    is_active          INTEGER NOT NULL DEFAULT 1,
    hlc                TEXT NOT NULL DEFAULT '',
    deleted_at         TEXT NULL,
    sync_state         TEXT NOT NULL DEFAULT 'pending',
    created_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_trading_principles_account ON trading_principles (account_id, deleted_at);

-- Per-account cursor for the principle pull (independent of `sync_meta`/
-- `journal_sync`), same pattern as `journal_sync`.
CREATE TABLE IF NOT EXISTS principle_sync (
    account_id   TEXT PRIMARY KEY NOT NULL,
    cookie       TEXT NULL,
    last_sync_at TEXT NULL
);

-- Position calculator. Rules are per-account: `account_id` is the PK (one row
-- per account), matching the server's `UNIQUE(user_id, account_id)` — `id` is
-- carried alongside for wire parity but is never the local key. Plans and
-- history are user-scoped (no account_id), same shape as playbooks/tags.
-- Whole-row LWW via a single `hlc`; soft-delete via `deleted_at`.
CREATE TABLE IF NOT EXISTS calc_rules (
    account_id        TEXT PRIMARY KEY NOT NULL,
    id                TEXT NOT NULL,
    account_balance   REAL NOT NULL,
    account_risk      REAL NOT NULL,
    max_stop_loss_pct REAL NOT NULL,
    hlc               TEXT NOT NULL DEFAULT '',
    deleted_at        TEXT NULL,
    sync_state        TEXT NOT NULL DEFAULT 'pending'
);

-- `tranches_json` is opaque here: an array of {id, percent, shares,
-- targetPrice, status, filledAt}, forwarded verbatim to/from the server. Only
-- the command layer (journal::calculator) ever parses it, and only to shape
-- the return value.
CREATE TABLE IF NOT EXISTS calc_plans (
    id               TEXT PRIMARY KEY NOT NULL,
    symbol           TEXT NOT NULL,
    position_type    TEXT NOT NULL,
    entry_price      REAL NOT NULL,
    stop_loss        REAL NOT NULL,
    account_balance  REAL NOT NULL,
    account_risk     REAL NOT NULL,
    total_shares     REAL NOT NULL,
    position_value   REAL NOT NULL,
    status           TEXT NOT NULL DEFAULT 'active',
    tranches_json    TEXT NOT NULL DEFAULT '[]',
    notes            TEXT NULL,
    hlc              TEXT NOT NULL DEFAULT '',
    deleted_at       TEXT NULL,
    sync_state       TEXT NOT NULL DEFAULT 'pending',
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_calc_plans_live ON calc_plans (deleted_at);

-- Append-only in practice (no update mutation on the server), but still gets
-- the same whole-row LWW/tombstone treatment as every other synced entity.
CREATE TABLE IF NOT EXISTS calc_history (
    id               TEXT PRIMARY KEY NOT NULL,
    symbol           TEXT NOT NULL,
    position_type    TEXT NOT NULL,
    entry_price      REAL NOT NULL,
    stop_loss        REAL NOT NULL,
    account_balance  REAL NOT NULL,
    account_risk     REAL NOT NULL,
    shares           REAL NOT NULL,
    position_value   REAL NOT NULL,
    account_pct      REAL NOT NULL,
    stop_loss_pct    REAL NOT NULL,
    hlc              TEXT NOT NULL DEFAULT '',
    deleted_at       TEXT NULL,
    sync_state       TEXT NOT NULL DEFAULT 'pending',
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_calc_history_live ON calc_history (deleted_at);

-- Single-row cursor for the user-scoped calculator pull. Rules + plans +
-- history share one cookie/cursor (like tags' categories+tags), separate from
-- every other cursor above.
CREATE TABLE IF NOT EXISTS calculator_sync (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    cookie       TEXT NULL,
    last_sync_at TEXT NULL
);
