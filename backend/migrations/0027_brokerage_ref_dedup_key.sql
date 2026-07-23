-- Fold `external_reference_id` into the dedup signature and collapse the rows
-- that its absence duplicated.
--
-- Migration 0025 left the reference id out, on the reading that SnapTrade shares
-- one across an order's buy/fee/fx rows -- citing three "partial fills" of one
-- option order carrying a single reference. Those three rows were not partial
-- fills. They carry three different `created_at` values from three separate sync
-- runs, back when the upsert still keyed on `snaptrade_id` (which SnapTrade
-- regenerates per fetch). One fill, imported three times. Genuine partial fills
-- arrive in a single fetch and land in one INSERT sharing a `created_at` to the
-- microsecond, and every real multi-fill group in the data carries a distinct
-- reference id per row.
--
-- The ordinal is the deeper problem: assigned by position in the fetched batch,
-- it makes the key a function of the row *and its neighbours*. Bust one of three
-- same-signature fills and the next sync numbers the survivors :0 and :1,
-- stranding :2 as a phantom row forever; return a batch in a different order and
-- the ordinals swap, trading fee/amount/settlement values between rows.
--
-- Keying on the brokerage's own reference makes the signature a function of the
-- row alone. The ordinal remains only for rows that arrive without a reference,
-- where attributes are still all there is to go on.

CREATE TEMP TABLE tx_sig ON COMMIT DROP AS
SELECT
    id,
    user_id,
    account_id,
    created_at,
    NULLIF(external_reference_id, '') AS ref,
    md5(
        COALESCE(lower(symbol), '') || '|' ||
        COALESCE(to_char(trade_date AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS'), '') || '|' ||
        to_char(COALESCE(units, 0), 'FM9999999999990.00000000') || '|' ||
        to_char(COALESCE(price, 0), 'FM9999999999990.00000000') || '|' ||
        COALESCE(lower(transaction_type), '') || '|' ||
        COALESCE(external_reference_id, '')
    ) AS sig
FROM brokerage_transactions;

-- Only rows carrying a reference collapse. A refless group is indistinguishable
-- by attributes alone, so its members stay and keep their ordinals.
CREATE TEMP TABLE tx_dupe ON COMMIT DROP AS
WITH ranked AS (
    SELECT
        id,
        first_value(id) OVER w AS survivor_id,
        row_number() OVER w AS rn
    FROM tx_sig
    WHERE ref IS NOT NULL
    WINDOW w AS (PARTITION BY user_id, account_id, sig ORDER BY created_at, id)
)
SELECT id AS loser_id, survivor_id FROM ranked WHERE rn > 1;

CREATE TEMP TABLE tx_map ON COMMIT DROP AS
SELECT loser_id AS from_id, survivor_id AS to_id FROM tx_dupe
UNION
SELECT survivor_id, survivor_id FROM tx_dupe;

CREATE TABLE IF NOT EXISTS brokerage_transactions_dedup_archive
    (LIKE brokerage_transactions);
ALTER TABLE brokerage_transactions_dedup_archive
    ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS superseded_by TEXT;

INSERT INTO brokerage_transactions_dedup_archive
SELECT t.*, now(), d.survivor_id
FROM brokerage_transactions t
JOIN tx_dupe d ON d.loser_id = t.id;

-- `journal_brokerage_links` is UNIQUE on brokerage_transaction_id, so a collapsed
-- group can end up with at most one link. Keep the oldest and drop its copies
-- before repointing, or the UPDATE below trips that constraint.
CREATE TEMP TABLE link_keep ON COMMIT DROP AS
SELECT DISTINCT ON (m.to_id) m.to_id AS survivor_id, l.id AS link_id
FROM journal_brokerage_links l
JOIN tx_map m ON m.from_id = l.brokerage_transaction_id
ORDER BY m.to_id, l.created_at, l.id;

DELETE FROM journal_brokerage_links l
USING tx_map m
WHERE m.from_id = l.brokerage_transaction_id
  AND l.id NOT IN (SELECT link_id FROM link_keep);

UPDATE journal_brokerage_links l
   SET brokerage_transaction_id = k.survivor_id
  FROM link_keep k
 WHERE l.id = k.link_id;

DELETE FROM brokerage_transactions t
USING tx_dupe d
WHERE t.id = d.loser_id;

-- Rebuilt rather than updated in place: every key changes, and dropping the index
-- first also makes its recreation the proof that the new keys are unique.
DROP INDEX IF EXISTS idx_brokerage_tx_dedup;

WITH ranked AS (
    SELECT
        s.id,
        s.sig || ':' || (
            row_number() OVER (
                PARTITION BY s.user_id, s.account_id, s.sig
                ORDER BY s.created_at, s.id
            ) - 1
        ) AS key
    FROM tx_sig s
    WHERE NOT EXISTS (SELECT 1 FROM tx_dupe d WHERE d.loser_id = s.id)
)
UPDATE brokerage_transactions AS t
   SET dedup_key = r.key
  FROM ranked AS r
 WHERE t.id = r.id;

CREATE UNIQUE INDEX idx_brokerage_tx_dedup
    ON brokerage_transactions (user_id, account_id, dedup_key);
