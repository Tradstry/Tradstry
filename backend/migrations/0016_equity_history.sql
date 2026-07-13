-- Reconstructed account equity history.
--
-- `accounts.total_value` is overwritten on every sync, so the history is lost. We
-- rebuild it from `brokerage_transactions` and materialize it here.

-- Cached daily closes so each symbol-day is fetched from the price API once, ever.
CREATE TABLE IF NOT EXISTS price_history (
    symbol     TEXT NOT NULL,
    date       DATE NOT NULL,
    close      DOUBLE PRECISION NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (symbol, date)
);

-- The materialized reconstruction. Rebuilt wholesale per account after a sync.
CREATE TABLE IF NOT EXISTS account_equity_history (
    user_id                 TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id              TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    date                    DATE NOT NULL,
    cash                    DOUBLE PRECISION NOT NULL,
    positions_value         DOUBLE PRECISION NOT NULL,
    equity                  DOUBLE PRECISION NOT NULL,
    net_contributions       DOUBLE PRECISION NOT NULL,
    funding_adjusted_equity DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (account_id, date)
);
CREATE INDEX IF NOT EXISTS idx_aeh_account_date ON account_equity_history (account_id, date);

-- Health of the last rebuild. `drift` is the acceptance test: our value for today
-- vs the broker's own `accounts.total_value`. A silently-wrong equity curve is worse
-- than no curve, so this is surfaced to the UI rather than hidden.
CREATE TABLE IF NOT EXISTS account_equity_rebuild (
    account_id           TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    user_id              TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rebuilt_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    reconstructed_equity DOUBLE PRECISION,
    reported_equity      DOUBLE PRECISION,
    drift                DOUBLE PRECISION,
    health_json          TEXT NOT NULL DEFAULT '{}'
);
