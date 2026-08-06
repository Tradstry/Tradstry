CREATE TABLE market_watchlists (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT market_watchlists_workspace_owner_fk
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX market_watchlists_workspace_name
    ON market_watchlists (workspace_id, lower(name));

CREATE TABLE market_watchlist_symbols (
    watchlist_id TEXT NOT NULL REFERENCES market_watchlists(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (watchlist_id, symbol),
    CONSTRAINT market_watchlist_symbols_symbol_check
        CHECK (symbol = upper(symbol) AND length(symbol) BETWEEN 1 AND 20)
);

CREATE TABLE market_reports (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    sources JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT market_reports_workspace_owner_fk
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE
);

CREATE INDEX market_reports_workspace_created
    ON market_reports (workspace_id, created_at DESC);

CREATE TABLE market_monitors (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    name TEXT NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('ABOVE', 'BELOW')),
    threshold DOUBLE PRECISION NOT NULL CHECK (threshold > 0),
    enabled BOOLEAN NOT NULL DEFAULT true,
    last_triggered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT market_monitors_workspace_owner_fk
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE
);

CREATE INDEX market_monitors_workspace_symbol
    ON market_monitors (workspace_id, symbol) WHERE enabled;

CREATE TRIGGER trg_market_watchlists_updated_at
    BEFORE UPDATE ON market_watchlists FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_market_monitors_updated_at
    BEFORE UPDATE ON market_monitors FOR EACH ROW EXECUTE FUNCTION set_updated_at();
