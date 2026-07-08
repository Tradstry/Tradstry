-- Make stop_loss / risk_reward nullable so NULL means "no stop / no R recorded"
-- (replacing the 0.0 sentinel). Backfill existing 0-stop rows to NULL, and null
-- risk_reward for those same rows (a no-stop trade has no determinable R).

ALTER TABLE journal_entries ALTER COLUMN stop_loss DROP NOT NULL;
ALTER TABLE journal_entries ALTER COLUMN risk_reward DROP NOT NULL;

UPDATE journal_entries SET stop_loss = NULL WHERE stop_loss = 0;
UPDATE journal_entries SET risk_reward = NULL WHERE stop_loss IS NULL;
