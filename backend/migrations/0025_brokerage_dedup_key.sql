-- Give brokerage_transactions a dedup key that survives SnapTrade re-registration.
--
-- The upsert previously keyed on `snaptrade_id`, which SnapTrade regenerates
-- whenever a user is deleted and re-registered (the documented recovery when a
-- userSecret is lost). The same fill came back under a new id and was INSERTed
-- rather than UPDATEd, duplicating history and orphaning the
-- journal_brokerage_links that pointed at the old rows.
--
-- The key is built only from the trade's own attributes, so nothing in it comes
-- from SnapTrade's identity and re-registration cannot change it.
--
-- `external_reference_id` is deliberately NOT used: SnapTrade documents it as
-- shared across the transactions of one order ("buy, fee, fx ... they can be
-- grouped if they share the same external_reference_id"), and real data has
-- three partial fills of one option order carrying one reference id.
--
-- Those partial fills are also identical in every broker-provided field —
-- same symbol, units, price, and trade_date to the microsecond — so attributes
-- alone cannot separate them either. The trailing ordinal handles that: N
-- indistinguishable fills become N rows keyed :0, :1, :2. Because the rows are
-- interchangeable, it does not matter which one a resync maps onto which.

ALTER TABLE brokerage_transactions ADD COLUMN IF NOT EXISTS dedup_key TEXT;

WITH signed AS (
    SELECT
        id,
        md5(
            COALESCE(lower(symbol), '') || '|' ||
            COALESCE(to_char(trade_date AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS'), '') || '|' ||
            to_char(COALESCE(units, 0), 'FM9999999999990.00000000') || '|' ||
            to_char(COALESCE(price, 0), 'FM9999999999990.00000000') || '|' ||
            COALESCE(lower(transaction_type), '')
        ) AS sig,
        user_id,
        account_id,
        snaptrade_id
    FROM brokerage_transactions
    WHERE dedup_key IS NULL
),
numbered AS (
    -- snaptrade_id only orders the tie; the rows it separates are identical, so
    -- any stable ordering produces an equivalent assignment.
    SELECT id, sig || ':' || (
        row_number() OVER (PARTITION BY user_id, account_id, sig ORDER BY snaptrade_id) - 1
    ) AS key
    FROM signed
)
UPDATE brokerage_transactions AS t
   SET dedup_key = n.key
  FROM numbered AS n
 WHERE t.id = n.id;

ALTER TABLE brokerage_transactions ALTER COLUMN dedup_key SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_brokerage_tx_dedup
    ON brokerage_transactions (user_id, account_id, dedup_key);
