-- Stop retrying webhooks that can never succeed.
--
-- Without this, an event that always fails (an unrecognised price_id, a payload
-- shape we don't model) is retried by the worker every 5 seconds forever —
-- thousands of identical error logs a day and no way to notice the ones that
-- matter. Past MAX_ATTEMPTS the row is left in place, unprocessed, for
-- inspection; it simply stops being claimed.

ALTER TABLE paddle_webhook_events
  ADD COLUMN IF NOT EXISTS attempts INTEGER NOT NULL DEFAULT 0;

-- The worker claims on (processed_at IS NULL AND attempts < N) ordered by
-- occurred_at, so the partial index needs to carry attempts too.
DROP INDEX IF EXISTS paddle_webhook_events_unprocessed_idx;

CREATE INDEX IF NOT EXISTS paddle_webhook_events_claimable_idx
  ON paddle_webhook_events (occurred_at)
  INCLUDE (attempts)
  WHERE processed_at IS NULL;
