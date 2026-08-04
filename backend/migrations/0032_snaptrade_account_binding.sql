ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS snaptrade_account_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_user_snaptrade_account
    ON accounts (user_id, snaptrade_account_id)
    WHERE snaptrade_account_id IS NOT NULL;
