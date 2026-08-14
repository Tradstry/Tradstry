-- Manual execution claims preserve user-entered execution details without
-- treating them as broker-verified fills.
CREATE TABLE IF NOT EXISTS manual_execution_claims (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    plan_id TEXT NOT NULL REFERENCES position_calculator_plans(id) ON DELETE CASCADE,
    tranche_id TEXT NOT NULL,
    quantity NUMERIC NOT NULL CHECK (quantity > 0),
    price NUMERIC NOT NULL CHECK (price > 0),
    executed_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'reconciled', 'dismissed')),
    reconciled_match_id TEXT REFERENCES trade_episode_matches(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_manual_execution_claims_active_tranche
    ON manual_execution_claims (user_id, plan_id, tranche_id)
    WHERE status IN ('pending', 'reconciled');

CREATE INDEX IF NOT EXISTS idx_manual_execution_claims_workspace
    ON manual_execution_claims (user_id, workspace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_manual_execution_claims_plan_status
    ON manual_execution_claims (plan_id, status, executed_at);
