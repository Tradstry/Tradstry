-- Plan entitlements: free / pro / pro_plus, differing only by numeric limits.
--
-- Plan state hangs off `users` (which already maps clerk_uuid -> internal id).
-- Limits live in `plan_limits` as DATA so changing a tier is an UPDATE, never a
-- deploy. `usage_counters` meters AI actions per period. `paddle_webhook_events`
-- gives webhook delivery idempotency (event_id PK) plus raw-payload durability.

-- ── users: subscription + usage state ───────────────────────────────────────
ALTER TABLE users
  ADD COLUMN IF NOT EXISTS plan                   TEXT NOT NULL DEFAULT 'free',
  ADD COLUMN IF NOT EXISTS paddle_customer_id     TEXT,
  ADD COLUMN IF NOT EXISTS paddle_subscription_id TEXT,
  ADD COLUMN IF NOT EXISTS subscription_status    TEXT,
  -- occurred_at of the last Paddle event applied. Paddle does not guarantee
  -- delivery order, so an event older than this is ignored. Deliberately not
  -- users.updated_at, which any unrelated profile write would bump.
  ADD COLUMN IF NOT EXISTS subscription_updated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS current_period_start   TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS current_period_end     TIMESTAMPTZ,
  -- Set when a subscription goes past_due; the plan survives until this passes.
  ADD COLUMN IF NOT EXISTS grace_until            TIMESTAMPTZ,
  -- Day-of-month the free-tier quota window rolls on (from signup). Days 29-31
  -- clamp to the last day of shorter months at read time.
  ADD COLUMN IF NOT EXISTS quota_anchor_day       SMALLINT,
  ADD COLUMN IF NOT EXISTS data_bytes_used        BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS media_bytes_used       BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS usage_recomputed_at    TIMESTAMPTZ;

UPDATE users
   SET quota_anchor_day = EXTRACT(DAY FROM created_at)::smallint
 WHERE quota_anchor_day IS NULL;

-- One Paddle subscription maps to at most one user.
CREATE UNIQUE INDEX IF NOT EXISTS users_paddle_subscription_id_key
  ON users (paddle_subscription_id)
  WHERE paddle_subscription_id IS NOT NULL;

-- ── plan_limits: entitlements as data ───────────────────────────────────────
-- NULL means unlimited. No tier uses NULL today; it exists so a future
-- lifetime/comp plan is a row rather than a code change.
CREATE TABLE IF NOT EXISTS plan_limits (
    plan                  TEXT PRIMARY KEY,
    ai_actions_per_month  INTEGER,
    brokerage_connections INTEGER,
    data_bytes            BIGINT,
    media_bytes           BIGINT
);

INSERT INTO plan_limits (plan, ai_actions_per_month, brokerage_connections, data_bytes, media_bytes)
VALUES
    ('free',       25,  1, (100  * 1024 * 1024)::bigint, (50  * 1024 * 1024)::bigint),
    ('pro',       300,  3, (700  * 1024 * 1024)::bigint, (300 * 1024 * 1024)::bigint),
    ('pro_plus', 1500, 10, (1500 * 1024 * 1024)::bigint, (800 * 1024 * 1024)::bigint)
ON CONFLICT (plan) DO NOTHING;

-- ── usage_counters: the AI meter ────────────────────────────────────────────
-- One row per (user, metric, window). Reservation is a single atomic statement
-- so check-and-increment cannot race; see billing_table::reserve_counter.
CREATE TABLE IF NOT EXISTS usage_counters (
    user_id      TEXT        NOT NULL,
    metric       TEXT        NOT NULL,
    period_start TIMESTAMPTZ NOT NULL,
    period_end   TIMESTAMPTZ NOT NULL,
    used         INTEGER     NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, metric, period_start)
);

-- ── paddle_webhook_events: idempotency + durability ─────────────────────────
-- The route persists here and returns 200 inside Paddle's 5s budget; a leased
-- worker processes rows where processed_at IS NULL. event_id as PK makes
-- redelivery (Paddle retries up to 60x) a no-op.
CREATE TABLE IF NOT EXISTS paddle_webhook_events (
    event_id     TEXT PRIMARY KEY,
    event_type   TEXT        NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL,
    payload      JSONB       NOT NULL,
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    error        TEXT
);

CREATE INDEX IF NOT EXISTS paddle_webhook_events_unprocessed_idx
  ON paddle_webhook_events (occurred_at)
  WHERE processed_at IS NULL;
