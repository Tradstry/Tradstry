-- Scheduled feedback: a worker fires per-user digests on a wall-clock boundary
-- rather than in reaction to an event.
--
-- Times are minutes since local midnight rather than TIME. The comparison the
-- worker makes is integer arithmetic against a computed local minute-of-day, and
-- TIME invites an accidental timezone conversion in the driver.

CREATE TABLE IF NOT EXISTS notification_user_settings (
    user_id              TEXT        PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    timezone             TEXT        NOT NULL DEFAULT 'America/New_York',
    daily_recap_minute   SMALLINT    NOT NULL DEFAULT 975,
    weekly_review_dow    SMALLINT    NOT NULL DEFAULT 0,
    weekly_review_minute SMALLINT    NOT NULL DEFAULT 1020,
    quiet_start_minute   SMALLINT,
    quiet_end_minute     SMALLINT,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE TRIGGER trg_notification_user_settings_updated_at
    BEFORE UPDATE ON notification_user_settings FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- One row per fired slot. The worker claims a slot with ON CONFLICT DO NOTHING
-- and only produces a notification when the insert reports a row, so a restart
-- or a slow tick inside the tolerance window cannot double-send.
--
-- local_date is DATE against the usual TIMESTAMPTZ rule: it is not an instant
-- but a calendar-day slot key in the user's own zone, and a TIMESTAMPTZ would
-- reintroduce the ambiguity the key exists to remove.
CREATE TABLE IF NOT EXISTS notification_schedule_runs (
    user_id    TEXT        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind       TEXT        NOT NULL,
    local_date DATE        NOT NULL,
    fired_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, kind, local_date)
);

CREATE INDEX IF NOT EXISTS idx_notification_schedule_runs_user
    ON notification_schedule_runs (user_id, kind);
