-- Offline-first position-calculator sync: a client-minted HLC stamp for
-- whole-row LWW and a soft-delete tombstone, mirroring 0011/0013/0014. Three
-- entities: rules (per user+account, whole-row upsert), plans (per-user
-- create + partial update + soft-delete), history (per-user append +
-- soft-delete, never updated).

ALTER TABLE position_calculator_rules ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE position_calculator_rules ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';

ALTER TABLE position_calculator_plans ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE position_calculator_plans ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';

-- `position_calculator_history` never had an `updated_at`/trigger (it's
-- append + soft-delete only, so nothing used to mutate it after insert); the
-- sync cursor needs one now that soft-delete does.
ALTER TABLE position_calculator_history ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE position_calculator_history ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE position_calculator_history ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';

CREATE OR REPLACE TRIGGER trg_pch_updated_at
    BEFORE UPDATE ON position_calculator_history FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- User-scoped pull cursor scan.
CREATE INDEX IF NOT EXISTS idx_pcr_cursor ON position_calculator_rules (user_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_pcp_cursor ON position_calculator_plans (user_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_pch_cursor ON position_calculator_history (user_id, updated_at);
