-- Backfill option-contract fields for transactions synced before option_symbol
-- parsing existed: their `symbol` is blank and `contract_multiplier` defaulted
-- to 1, so they were dropped from pending trades and priced as equities.
-- Incremental sync never re-fetches these rows, so they cannot self-heal — but
-- each still carries the full SnapTrade activity in `raw_json`, so we re-derive
-- the contract from there.
--
-- Idempotent and bounded: only blank option rows whose raw_json is a JSON object
-- carrying an `option_symbol`. Once a row is populated its `symbol` is no longer
-- blank, so re-running matches nothing. The MATERIALIZED CTE guarantees the
-- `raw_json::jsonb` cast only runs on rows that already look like JSON objects.
WITH candidates AS MATERIALIZED (
    SELECT id, raw_json::jsonb AS activity
    FROM brokerage_transactions
    WHERE COALESCE(option_type, '') <> ''
      AND COALESCE(symbol, '') = ''
      AND btrim(raw_json) LIKE '{%'
)
UPDATE brokerage_transactions AS t SET
    symbol            = c.activity -> 'option_symbol' ->> 'ticker',
    underlying_symbol = c.activity -> 'option_symbol' -> 'underlying_symbol' ->> 'symbol',
    option_kind       = c.activity -> 'option_symbol' ->> 'option_type',
    strike_price      = (c.activity -> 'option_symbol' ->> 'strike_price')::double precision,
    option_expiration = c.activity -> 'option_symbol' ->> 'expiration_date',
    contract_multiplier = CASE
        WHEN (c.activity -> 'option_symbol' ->> 'is_mini_option')::boolean THEN 10
        ELSE 100
    END,
    symbol_description = concat_ws(
        ' ',
        c.activity -> 'option_symbol' -> 'underlying_symbol' ->> 'symbol',
        '$' || (c.activity -> 'option_symbol' ->> 'strike_price'),
        initcap(c.activity -> 'option_symbol' ->> 'option_type'),
        c.activity -> 'option_symbol' ->> 'expiration_date'
    )
FROM candidates AS c
WHERE t.id = c.id
  AND jsonb_typeof(c.activity -> 'option_symbol') = 'object';
