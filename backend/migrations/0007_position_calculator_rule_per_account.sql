-- The rule was one row per user. Balance now comes from the selected account,
-- and risk tolerance differs between a main portfolio and a paper account, so
-- the rule becomes one row per (user, account).
ALTER TABLE position_calculator_rules
    ADD COLUMN IF NOT EXISTS account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE;

-- Backfill each existing rule onto its user's oldest account. The `id` tiebreak
-- keeps the choice deterministic when two accounts share a created_at.
UPDATE position_calculator_rules r
SET account_id = (
    SELECT a.id FROM accounts a
    WHERE a.user_id = r.user_id
    ORDER BY a.created_at ASC, a.id ASC
    LIMIT 1
)
WHERE r.account_id IS NULL;

-- A rule belonging to a user with no account cannot be scoped to one.
DELETE FROM position_calculator_rules WHERE account_id IS NULL;

ALTER TABLE position_calculator_rules ALTER COLUMN account_id SET NOT NULL;

ALTER TABLE position_calculator_rules
    DROP CONSTRAINT IF EXISTS position_calculator_rules_user_id_key;

CREATE UNIQUE INDEX IF NOT EXISTS idx_pcr_user_account
    ON position_calculator_rules (user_id, account_id);
CREATE INDEX IF NOT EXISTS idx_pcr_account
    ON position_calculator_rules (account_id);
