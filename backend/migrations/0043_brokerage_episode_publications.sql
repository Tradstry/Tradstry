-- One brokerage episode can produce at most one journal entry. This keeps the
-- review action idempotent even when the user retries after a network failure.
CREATE TABLE IF NOT EXISTS brokerage_episode_publications (
    episode_id TEXT PRIMARY KEY REFERENCES trade_episodes(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    journal_entry_id TEXT NOT NULL UNIQUE REFERENCES journal_entries(id) ON DELETE CASCADE,
    plan_id TEXT REFERENCES position_calculator_plans(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brokerage_episode_publications_workspace
    ON brokerage_episode_publications (user_id, workspace_id, created_at DESC);
