-- Notification engine: producers record raw events, a worker renders them into
-- user-facing rows, a second worker pushes them to browsers.
--
-- Outbox rather than a direct call so a producer never evaluates preferences,
-- renders copy, or waits on a push service. Adding an event type touches the
-- renderer only.

CREATE TABLE IF NOT EXISTS notification_outbox (
    -- BIGSERIAL, not TEXT: the worker consumes strictly in insertion order.
    id           BIGSERIAL   PRIMARY KEY,
    user_id      TEXT        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type   TEXT        NOT NULL,
    payload      JSONB       NOT NULL,
    -- NULL means this event is never folded into a group.
    coalesce_key TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    attempts     INT         NOT NULL DEFAULT 0,
    last_error   TEXT
);

-- Partial: the hot query only ever scans pending rows, so the index stays small
-- however much history accumulates.
CREATE INDEX IF NOT EXISTS idx_notification_outbox_pending
    ON notification_outbox (id) WHERE processed_at IS NULL;

CREATE TABLE IF NOT EXISTS notifications (
    id             TEXT        PRIMARY KEY NOT NULL,
    user_id        TEXT        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type     TEXT        NOT NULL,
    title          TEXT        NOT NULL,
    body           TEXT        NOT NULL DEFAULT '',
    deep_link      TEXT,
    payload        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    coalesce_key   TEXT,
    group_count    INT         NOT NULL DEFAULT 1,
    read_at        TIMESTAMPTZ,
    last_pushed_at TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The feed sorts on updated_at so a freshly-grouped notification floats back up.
CREATE INDEX IF NOT EXISTS idx_notifications_feed
    ON notifications (user_id, updated_at DESC);

-- The unread badge is polled far more often than the feed is opened.
CREATE INDEX IF NOT EXISTS idx_notifications_unread
    ON notifications (user_id) WHERE read_at IS NULL;

-- The coalescing target. Without UNIQUE, two workers racing on one key create
-- duplicate groups; the partial predicate is what lets a new group start once
-- the previous one has been read.
CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_coalesce
    ON notifications (user_id, coalesce_key)
    WHERE read_at IS NULL AND coalesce_key IS NOT NULL;

CREATE OR REPLACE TRIGGER trg_notifications_updated_at
    BEFORE UPDATE ON notifications FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Stored only as deviations from the default: a missing row means enabled. That
-- is what lets a new event type reach every existing user with no backfill.
CREATE TABLE IF NOT EXISTS notification_preferences (
    user_id    TEXT        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type TEXT        NOT NULL,
    enabled    BOOLEAN     NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, event_type)
);

CREATE OR REPLACE TRIGGER trg_notification_preferences_updated_at
    BEFORE UPDATE ON notification_preferences FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id               TEXT        PRIMARY KEY NOT NULL,
    user_id          TEXT        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Unique so re-subscribing the same browser upserts instead of duplicating.
    endpoint         TEXT        NOT NULL UNIQUE,
    p256dh           TEXT        NOT NULL,
    auth             TEXT        NOT NULL,
    user_agent       TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_success_at  TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_push_subscriptions_user
    ON push_subscriptions (user_id);

CREATE TABLE IF NOT EXISTS notification_deliveries (
    notification_id TEXT        NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    subscription_id TEXT        NOT NULL REFERENCES push_subscriptions(id) ON DELETE CASCADE,
    status          TEXT        NOT NULL DEFAULT 'pending',
    attempts        INT         NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error      TEXT,
    sent_at         TIMESTAMPTZ,
    PRIMARY KEY (notification_id, subscription_id),
    CONSTRAINT notification_deliveries_status_check
        CHECK (status IN ('pending', 'sent', 'failed', 'gone'))
);

CREATE INDEX IF NOT EXISTS idx_notification_deliveries_due
    ON notification_deliveries (next_attempt_at) WHERE status = 'pending';
