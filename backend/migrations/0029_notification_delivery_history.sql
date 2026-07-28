-- Keep delivery history when a browser goes away.
--
-- 0028 gave notification_deliveries.subscription_id a cascading foreign key, so
-- deleting a push subscription deleted every delivery row that referenced it.
-- Two consequences, both wrong: removing a browser in settings silently erased
-- the record that its pushes had ever been sent, and the 'gone' status the
-- delivery worker writes for a dead endpoint (404/410) was deleted in the same
-- breath -- a state no query could ever observe.
--
-- The column stays; only the constraint goes. `claim_due` inner-joins
-- push_subscriptions, so a row whose subscription is gone drops out of the work
-- queue on its own, and the retention sweep clears it later.

ALTER TABLE notification_deliveries
    DROP CONSTRAINT IF EXISTS notification_deliveries_subscription_id_fkey;
