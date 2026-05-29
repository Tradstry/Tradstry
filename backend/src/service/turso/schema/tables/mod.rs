pub mod accounts_table;
pub mod brokerage_table;
pub mod journal_table;
pub mod notebook_folders_table;
pub mod notebook_images;
pub mod notebook_table;
pub mod playbook_table;
pub mod position_calculator_history_table;
pub mod position_calculator_plans_table;
pub mod position_calculator_rule_table;
pub mod tags_table;
pub mod user_agents_table;
pub mod user_prompts_table;
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
    snaptrade_user_id TEXT,
    snaptrade_user_secret_encrypted TEXT,
    snaptrade_connection_id TEXT,
    total_value REAL,
    total_value_currency TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_accounts_user_id ON accounts (user_id);

CREATE TABLE IF NOT EXISTS brokerage_transactions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    snaptrade_id TEXT NOT NULL,
    symbol TEXT,
    symbol_description TEXT,
    raw_symbol TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    transaction_type TEXT NOT NULL,
    option_type TEXT,
    price REAL NOT NULL DEFAULT 0,
    units REAL NOT NULL DEFAULT 0,
    amount REAL,
    fee REAL NOT NULL DEFAULT 0,
    fx_rate REAL,
    description TEXT,
    trade_date TEXT,
    settlement_date TEXT NOT NULL,
    institution TEXT NOT NULL,
    external_reference_id TEXT,
    raw_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (user_id, account_id, snaptrade_id)
);

CREATE INDEX IF NOT EXISTS idx_brokerage_tx_user_account ON brokerage_transactions (user_id, account_id);
CREATE INDEX IF NOT EXISTS idx_brokerage_tx_symbol ON brokerage_transactions (symbol);
CREATE INDEX IF NOT EXISTS idx_brokerage_tx_trade_date ON brokerage_transactions (trade_date);
CREATE INDEX IF NOT EXISTS idx_brokerage_tx_type ON brokerage_transactions (transaction_type);

CREATE TABLE IF NOT EXISTS brokerage_holdings (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    snaptrade_symbol_id TEXT,
    symbol TEXT NOT NULL,
    symbol_description TEXT,
    raw_symbol TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    units REAL NOT NULL DEFAULT 0,
    price REAL NOT NULL DEFAULT 0,
    market_value REAL,
    open_pnl REAL,
    average_purchase_price REAL,
    is_option BOOLEAN NOT NULL DEFAULT false,
    option_type TEXT,
    strike_price REAL,
    expiration_date TEXT,
    raw_json TEXT NOT NULL,
    synced_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (user_id, account_id, symbol)
);

CREATE INDEX IF NOT EXISTS idx_brokerage_holdings_user_account ON brokerage_holdings (user_id, account_id);
CREATE INDEX IF NOT EXISTS idx_brokerage_holdings_symbol ON brokerage_holdings (symbol);

CREATE TABLE IF NOT EXISTS brokerage_balances (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    currency TEXT NOT NULL DEFAULT 'USD',
    cash REAL,
    buying_power REAL,
    synced_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (user_id, account_id, currency)
);

CREATE INDEX IF NOT EXISTS idx_brokerage_balances_user_account ON brokerage_balances (user_id, account_id);

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

CREATE TABLE IF NOT EXISTS notebook_folders (
    id               TEXT PRIMARY KEY NOT NULL,
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id       TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    parent_folder_id TEXT REFERENCES notebook_folders(id) ON DELETE CASCADE,
    name             TEXT NOT NULL,
    sort_order       INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_notebook_folders_account_id ON notebook_folders (account_id);
CREATE INDEX IF NOT EXISTS idx_notebook_folders_parent_folder_id ON notebook_folders (parent_folder_id);

CREATE TABLE IF NOT EXISTS notebook_notes (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_id TEXT REFERENCES notebook_folders(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
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
    media_type TEXT NOT NULL DEFAULT 'image',
    content_type TEXT NOT NULL DEFAULT '',
    duration_seconds REAL NOT NULL DEFAULT 0,
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

CREATE TRIGGER IF NOT EXISTS trg_notebook_folders_updated_at
AFTER UPDATE ON notebook_folders
FOR EACH ROW
BEGIN
    UPDATE notebook_folders SET updated_at = datetime('now') WHERE id = OLD.id;
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

CREATE TRIGGER IF NOT EXISTS trg_brokerage_transactions_updated_at
AFTER UPDATE ON brokerage_transactions
FOR EACH ROW
BEGIN
    UPDATE brokerage_transactions SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_brokerage_holdings_updated_at
AFTER UPDATE ON brokerage_holdings
FOR EACH ROW
BEGIN
    UPDATE brokerage_holdings SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_brokerage_balances_updated_at
AFTER UPDATE ON brokerage_balances
FOR EACH ROW
BEGIN
    UPDATE brokerage_balances SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TABLE IF NOT EXISTS journal_brokerage_links (
    id TEXT PRIMARY KEY NOT NULL,
    journal_entry_id TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
    brokerage_transaction_id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_jbl_journal_entry ON journal_brokerage_links(journal_entry_id);
CREATE INDEX IF NOT EXISTS idx_jbl_user ON journal_brokerage_links(user_id);
CREATE INDEX IF NOT EXISTS idx_jbl_brokerage_tx ON journal_brokerage_links(brokerage_transaction_id);

CREATE TABLE IF NOT EXISTS user_agents (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    goal TEXT NOT NULL,
    steps_json TEXT NOT NULL,
    output_style TEXT NOT NULL DEFAULT 'concise',
    config_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_user_agents_user_id ON user_agents (user_id);
CREATE INDEX IF NOT EXISTS idx_user_agents_account ON user_agents (user_id, account_id);

CREATE TABLE IF NOT EXISTS user_prompts (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_user_prompts_user_id ON user_prompts (user_id);

CREATE TRIGGER IF NOT EXISTS trg_user_prompts_updated_at
AFTER UPDATE ON user_prompts
FOR EACH ROW
BEGIN
    UPDATE user_prompts SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TABLE IF NOT EXISTS position_calculator_rules (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    account_balance REAL NOT NULL,
    account_risk REAL NOT NULL,
    max_stop_loss_pct REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TRIGGER IF NOT EXISTS trg_position_calculator_rules_updated_at
AFTER UPDATE ON position_calculator_rules
FOR EACH ROW
BEGIN
    UPDATE position_calculator_rules SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TABLE IF NOT EXISTS position_calculator_history (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    position_type TEXT NOT NULL CHECK (position_type IN ('long', 'short')),
    entry_price REAL NOT NULL,
    stop_loss REAL NOT NULL,
    account_balance REAL NOT NULL,
    account_risk REAL NOT NULL,
    shares REAL NOT NULL,
    position_value REAL NOT NULL,
    account_pct REAL NOT NULL,
    stop_loss_pct REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_position_calc_history_user_id ON position_calculator_history (user_id);

CREATE TABLE IF NOT EXISTS position_calculator_plans (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    position_type TEXT NOT NULL CHECK (position_type IN ('long', 'short')),
    entry_price REAL NOT NULL,
    stop_loss REAL NOT NULL,
    account_balance REAL NOT NULL,
    account_risk REAL NOT NULL,
    total_shares REAL NOT NULL,
    position_value REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed', 'cancelled')),
    tranches_json TEXT NOT NULL DEFAULT '[]',
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_position_calc_plans_user_id ON position_calculator_plans (user_id);

CREATE TRIGGER IF NOT EXISTS trg_position_calculator_plans_updated_at
AFTER UPDATE ON position_calculator_plans
FOR EACH ROW
BEGIN
    UPDATE position_calculator_plans SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TABLE IF NOT EXISTS tag_categories (
    id TEXT PRIMARY KEY, user_id TEXT NOT NULL, name TEXT NOT NULL,
    role TEXT, color TEXT, sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL );

CREATE UNIQUE INDEX IF NOT EXISTS idx_tagcat_user_name ON tag_categories (user_id, lower(name));
CREATE UNIQUE INDEX IF NOT EXISTS idx_tagcat_user_role ON tag_categories (user_id, role) WHERE role IS NOT NULL;

CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY, user_id TEXT NOT NULL,
    category_id TEXT NOT NULL REFERENCES tag_categories(id) ON DELETE CASCADE,
    name TEXT NOT NULL, color TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL );

CREATE UNIQUE INDEX IF NOT EXISTS idx_tags_user_cat_name ON tags (user_id, category_id, lower(name));
CREATE INDEX IF NOT EXISTS idx_tags_category ON tags (category_id);

CREATE TABLE IF NOT EXISTS trade_tags (
    journal_entry_id TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (journal_entry_id, tag_id) );

CREATE INDEX IF NOT EXISTS idx_trade_tags_tag ON trade_tags (tag_id);
"#;
