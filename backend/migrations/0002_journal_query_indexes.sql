-- Indexes supporting the SQL-side filtering in `query_trades`
-- (`list_journal_entries_filtered`).

-- Case-insensitive symbol lookups: the filter predicate is `UPPER(symbol) = $n`,
-- which a plain btree on `symbol` cannot serve, so index the expression.
CREATE INDEX IF NOT EXISTS idx_journal_entries_symbol_upper
    ON journal_entries (UPPER(symbol));

-- Serves the default (and account-scoped) `WHERE user_id = $1 ORDER BY
-- open_date DESC, close_date DESC LIMIT n` without a full sort.
CREATE INDEX IF NOT EXISTS idx_journal_entries_user_open_close
    ON journal_entries (user_id, open_date DESC, close_date DESC);
