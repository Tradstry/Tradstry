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

CREATE TABLE IF NOT EXISTS ai_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    job_type TEXT NOT NULL,
    artifact_type TEXT,
    time_filter_json TEXT NOT NULL DEFAULT '{}',
    payload_json TEXT NOT NULL,
    dedupe_key TEXT,
    status TEXT NOT NULL DEFAULT 'queued',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    lease_owner TEXT,
    leased_at TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_ai_jobs_status_created_at ON ai_jobs (status, created_at);
CREATE INDEX IF NOT EXISTS idx_ai_jobs_user_account ON ai_jobs (user_id, account_id);
CREATE INDEX IF NOT EXISTS idx_ai_jobs_dedupe_key ON ai_jobs (dedupe_key);

CREATE TABLE IF NOT EXISTS ai_source_documents (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    title TEXT NOT NULL,
    body_text TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    content_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (user_id, account_id, source_type, source_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_source_documents_user_account ON ai_source_documents (user_id, account_id);
CREATE INDEX IF NOT EXISTS idx_ai_source_documents_source ON ai_source_documents (source_type, source_id);

CREATE TABLE IF NOT EXISTS ai_artifacts (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    artifact_type TEXT NOT NULL,
    time_filter_json TEXT NOT NULL DEFAULT '{}',
    range_start TEXT,
    range_end TEXT,
    status TEXT NOT NULL DEFAULT 'queued',
    model TEXT NOT NULL DEFAULT '',
    prompt_version TEXT NOT NULL DEFAULT '',
    payload_json TEXT NOT NULL DEFAULT '{}',
    error_message TEXT,
    generated_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_ai_artifacts_user_account_type ON ai_artifacts (user_id, account_id, artifact_type);

CREATE TABLE IF NOT EXISTS ai_artifact_sources (
    id TEXT PRIMARY KEY NOT NULL,
    artifact_id TEXT NOT NULL REFERENCES ai_artifacts(id) ON DELETE CASCADE,
    source_document_id TEXT NOT NULL REFERENCES ai_source_documents(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    title TEXT NOT NULL,
    excerpt TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_ai_artifact_sources_artifact_id ON ai_artifact_sources (artifact_id);

CREATE TABLE IF NOT EXISTS chat_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    account_id TEXT NOT NULL REFERENCES accounts(id),
    title TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_account
    ON chat_sessions(user_id, account_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    context_json TEXT,
    tool_name TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session_created
    ON chat_messages(session_id, created_at DESC);

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

CREATE TRIGGER IF NOT EXISTS trg_ai_jobs_updated_at
AFTER UPDATE ON ai_jobs
FOR EACH ROW
BEGIN
    UPDATE ai_jobs SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_ai_source_documents_updated_at
AFTER UPDATE ON ai_source_documents
FOR EACH ROW
BEGIN
    UPDATE ai_source_documents SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_ai_artifacts_updated_at
AFTER UPDATE ON ai_artifacts
FOR EACH ROW
BEGIN
    UPDATE ai_artifacts SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_chat_sessions_updated_at
AFTER UPDATE ON chat_sessions
FOR EACH ROW
BEGIN
    UPDATE chat_sessions SET updated_at = datetime('now') WHERE id = OLD.id;
END;
"#;
