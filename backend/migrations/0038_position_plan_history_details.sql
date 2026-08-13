-- Preserve the resolved plan as an immutable history snapshot. The source plan
-- can continue to evolve independently without changing what was executed.
ALTER TABLE position_calculator_history
    ADD COLUMN IF NOT EXISTS plan_id TEXT,
    ADD COLUMN IF NOT EXISTS tranches_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX IF NOT EXISTS idx_position_calc_history_plan_id
    ON position_calculator_history (plan_id)
    WHERE plan_id IS NOT NULL;
