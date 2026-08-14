UPDATE brokerage_connections
SET last_sync_succeeded_at = last_sync_finished_at
WHERE last_sync_status = 'completed'
  AND last_sync_succeeded_at IS NULL
  AND last_sync_finished_at IS NOT NULL;

