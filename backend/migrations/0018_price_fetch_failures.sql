-- Delisted tickers never return candles, so "this symbol has no cached prices" stays true
-- forever and would force a price fetch on every sweep. Track the misses so a dead symbol
-- stops driving fetches, while still being retried occasionally in case the feed recovers.
CREATE TABLE IF NOT EXISTS price_fetch_failures (
    symbol TEXT PRIMARY KEY NOT NULL,
    misses INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
