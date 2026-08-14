-- Immutable plan-vs-actual reviews built from broker executions.
ALTER TABLE position_calculator_plans
    ADD COLUMN IF NOT EXISTS execution_check_requested_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS instrument_json JSONB;

CREATE TABLE IF NOT EXISTS trade_episodes (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    instrument_key TEXT NOT NULL,
    instrument_json JSONB NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('long', 'short')),
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ,
    current_quantity NUMERIC NOT NULL CHECK (current_quantity >= 0),
    status TEXT NOT NULL DEFAULT 'ready' CHECK (status IN ('ready', 'blocked')),
    block_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, workspace_id, fingerprint)
);
CREATE INDEX IF NOT EXISTS idx_trade_episodes_inbox
    ON trade_episodes (user_id, workspace_id, opened_at DESC);

CREATE TABLE IF NOT EXISTS trade_episode_fills (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL REFERENCES trade_episodes(id) ON DELETE CASCADE,
    brokerage_transaction_id TEXT NOT NULL REFERENCES brokerage_transactions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('entry', 'exit')),
    quantity NUMERIC NOT NULL CHECK (quantity > 0),
    price NUMERIC NOT NULL CHECK (price > 0),
    fee NUMERIC NOT NULL DEFAULT 0,
    executed_at TIMESTAMPTZ NOT NULL,
    UNIQUE (episode_id, brokerage_transaction_id, role)
);
CREATE INDEX IF NOT EXISTS idx_trade_episode_fills_episode
    ON trade_episode_fills (episode_id, executed_at, brokerage_transaction_id);

CREATE TABLE IF NOT EXISTS trade_episode_matches (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    episode_id TEXT NOT NULL REFERENCES trade_episodes(id) ON DELETE CASCADE,
    plan_id TEXT NOT NULL REFERENCES position_calculator_plans(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('suggested', 'confirmed', 'rejected')),
    score NUMERIC NOT NULL,
    evidence_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (episode_id, plan_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_trade_episode_one_confirmed_match
    ON trade_episode_matches (episode_id) WHERE status = 'confirmed';

CREATE TABLE IF NOT EXISTS trade_review_versions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    match_id TEXT NOT NULL REFERENCES trade_episode_matches(id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL CHECK (version_number > 0),
    stage TEXT NOT NULL CHECK (stage IN ('entry', 'final')),
    plan_snapshot_json JSONB NOT NULL,
    calculation_json JSONB NOT NULL,
    reflection_json JSONB,
    journal_draft_json JSONB,
    finalized_at TIMESTAMPTZ,
    supersedes_id TEXT REFERENCES trade_review_versions(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (match_id, version_number)
);
CREATE INDEX IF NOT EXISTS idx_trade_review_versions_match
    ON trade_review_versions (match_id, version_number DESC);

CREATE TABLE IF NOT EXISTS trade_review_publications (
    review_version_id TEXT PRIMARY KEY REFERENCES trade_review_versions(id) ON DELETE CASCADE,
    journal_entry_id TEXT NOT NULL UNIQUE REFERENCES journal_entries(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION reject_trade_review_version_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'trade review versions are immutable; create a correction version';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_trade_review_versions_immutable ON trade_review_versions;
CREATE TRIGGER trg_trade_review_versions_immutable
    BEFORE UPDATE ON trade_review_versions
    FOR EACH ROW EXECUTE FUNCTION reject_trade_review_version_update();
