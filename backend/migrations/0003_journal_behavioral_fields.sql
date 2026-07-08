-- Structured behavioral fields for `journal_entries`. All nullable, no default:
-- existing rows stay NULL and the API sets them explicitly when provided.

ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS broke_30min_rule BOOLEAN;
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS pre_trade_conviction INTEGER;
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS market_regime TEXT;
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS is_planned_pre_market BOOLEAN;
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS revenge_trade BOOLEAN;
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS rule_adherence_score INTEGER;
