pub mod accounts_table;
pub mod journal_table;
pub mod notebook_images;
pub mod notebook_table;
pub mod playbook_table;
pub mod users_table;

/// Define your schema here. Bump SCHEMA_VERSION in logic.rs when you change this.
///
/// The migrator will automatically:
///   - Create new tables
///   - Drop removed tables (safely)
///   - Add new columns
///   - Drop removed columns (via table rebuild)
///   - Rename columns (via `-- rename: old_name -> new_name` comments)
///   - Rename tables  (via `-- rename_table: old_name -> new_name` comments)
///   - Create/drop indexes
///   - Create/drop triggers
///
/// Use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` as usual.
pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    clerk_uuid TEXT NOT NULL UNIQUE,
    full_name TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_users_clerk_uuid ON users (clerk_uuid);

CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    icon TEXT NOT NULL DEFAULT 'chart-line-data-01',
    currency TEXT NOT NULL DEFAULT 'USD',
    broker TEXT,
    risk_profile TEXT NOT NULL DEFAULT 'moderate',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_accounts_user_id ON accounts (user_id);

CREATE TABLE IF NOT EXISTS journal_entries (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    reviewed BOOLEAN NOT NULL DEFAULT false,
    open_date TEXT NOT NULL,
    close_date TEXT NOT NULL,
    entry_price REAL NOT NULL,
    exit_price REAL NOT NULL,
    position_size REAL NOT NULL,
    symbol TEXT NOT NULL,
    symbol_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('profit', 'loss')),
    total_pl REAL NOT NULL,
    net_roi REAL NOT NULL,
    duration INTEGER NOT NULL,
    stop_loss REAL NOT NULL,
    risk_reward REAL NOT NULL,
    trade_type TEXT NOT NULL CHECK (trade_type IN ('long', 'short')),
    mistakes TEXT NOT NULL,
    entry_tactics TEXT NOT NULL,
    edges_spotted TEXT NOT NULL,
    playbook_id TEXT,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_journal_entries_user_id ON journal_entries (user_id);
CREATE INDEX IF NOT EXISTS idx_journal_entries_symbol ON journal_entries (symbol);
CREATE INDEX IF NOT EXISTS idx_journal_entries_playbook_id ON journal_entries (playbook_id);

CREATE TABLE IF NOT EXISTS playbooks (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    edge_name TEXT NOT NULL,
    entry_rules TEXT NOT NULL,
    exit_rules TEXT NOT NULL,
    position_sizing_rules TEXT NOT NULL,
    additional_rules TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_playbooks_user_id ON playbooks (user_id);

CREATE TABLE IF NOT EXISTS notebook_notes (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    document_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_notebook_notes_user_id ON notebook_notes (user_id);
CREATE INDEX IF NOT EXISTS idx_notebook_notes_account_id ON notebook_notes (account_id);

CREATE TABLE IF NOT EXISTS notebook_note_trades (
    note_id TEXT NOT NULL REFERENCES notebook_notes(id) ON DELETE CASCADE,
    trade_id TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (note_id, trade_id)
);

CREATE INDEX IF NOT EXISTS idx_notebook_note_trades_trade_id ON notebook_note_trades (trade_id);

CREATE TABLE IF NOT EXISTS notebook_images (
    id TEXT PRIMARY KEY NOT NULL,
    note_id TEXT NOT NULL REFERENCES notebook_notes(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    cloudinary_asset_id TEXT NOT NULL UNIQUE,
    cloudinary_public_id TEXT NOT NULL UNIQUE,
    secure_url TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    format TEXT NOT NULL DEFAULT '',
    bytes INTEGER NOT NULL DEFAULT 0,
    original_filename TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_notebook_images_note_id ON notebook_images (note_id);
CREATE INDEX IF NOT EXISTS idx_notebook_images_user_id ON notebook_images (user_id);
CREATE INDEX IF NOT EXISTS idx_notebook_images_account_id ON notebook_images (account_id);

CREATE TRIGGER IF NOT EXISTS trg_users_updated_at
AFTER UPDATE ON users
FOR EACH ROW
BEGIN
    UPDATE users SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_accounts_updated_at
AFTER UPDATE ON accounts
FOR EACH ROW
BEGIN
    UPDATE accounts SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_notebook_notes_updated_at
AFTER UPDATE ON notebook_notes
FOR EACH ROW
BEGIN
    UPDATE notebook_notes SET updated_at = datetime('now') WHERE id = OLD.id;
END;
"#;
