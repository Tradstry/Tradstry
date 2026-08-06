-- Promote the former app-level "account" container to an explicit workspace.
-- A workspace owns trading data and may have at most one brokerage connection.

ALTER TABLE accounts RENAME TO workspaces;

ALTER TABLE workspaces
    ADD COLUMN IF NOT EXISTS asset_class TEXT NOT NULL DEFAULT 'mixed';

ALTER TABLE workspaces
    ADD CONSTRAINT workspaces_asset_class_check
    CHECK (asset_class IN ('futures', 'options', 'stocks', 'forex', 'crypto', 'mixed', 'other'));

-- Defensive repair for legacy or administratively-created users that somehow
-- have no account row. The application normally created one at first sign-in.
INSERT INTO workspaces (id, user_id, name, asset_class)
SELECT gen_random_uuid()::text, u.id, 'Main Workspace', 'mixed'
FROM users u
WHERE NOT EXISTS (SELECT 1 FROM workspaces w WHERE w.user_id = u.id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_workspaces_id_user
    ON workspaces (id, user_id);

CREATE TABLE IF NOT EXISTS brokerage_connections (
    workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT 'snaptrade',
    broker TEXT,
    snaptrade_user_id TEXT,
    snaptrade_user_secret_encrypted TEXT,
    snaptrade_connection_id TEXT,
    snaptrade_account_id TEXT,
    total_value DOUBLE PRECISION,
    total_value_currency TEXT,
    connection_disabled BOOLEAN NOT NULL DEFAULT false,
    connection_disabled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT brokerage_connections_workspace_owner_fk
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_brokerage_connections_user_snaptrade_account
    ON brokerage_connections (user_id, snaptrade_account_id)
    WHERE snaptrade_account_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_brokerage_connections_user
    ON brokerage_connections (user_id);
CREATE OR REPLACE TRIGGER trg_brokerage_connections_updated_at
    BEFORE UPDATE ON brokerage_connections FOR EACH ROW EXECUTE FUNCTION set_updated_at();

INSERT INTO brokerage_connections (
    workspace_id, user_id, broker, snaptrade_user_id,
    snaptrade_user_secret_encrypted, snaptrade_connection_id,
    snaptrade_account_id, total_value, total_value_currency,
    connection_disabled, connection_disabled_at, created_at, updated_at
)
SELECT
    id, user_id, NULLIF(broker, ''), snaptrade_user_id,
    snaptrade_user_secret_encrypted, NULLIF(snaptrade_connection_id, ''),
    snaptrade_account_id, total_value, NULLIF(total_value_currency, ''),
    snaptrade_connection_disabled, snaptrade_connection_disabled_at,
    created_at, updated_at
FROM workspaces
WHERE NULLIF(broker, '') IS NOT NULL
   OR snaptrade_user_id IS NOT NULL
   OR snaptrade_connection_id IS NOT NULL
   OR snaptrade_account_id IS NOT NULL
   OR total_value IS NOT NULL
ON CONFLICT (workspace_id) DO NOTHING;

ALTER TABLE workspaces
    DROP COLUMN broker,
    DROP COLUMN snaptrade_user_id,
    DROP COLUMN snaptrade_user_secret_encrypted,
    DROP COLUMN snaptrade_connection_id,
    DROP COLUMN snaptrade_account_id,
    DROP COLUMN total_value,
    DROP COLUMN total_value_currency,
    DROP COLUMN snaptrade_connection_disabled,
    DROP COLUMN snaptrade_connection_disabled_at;

-- Existing account-scoped records become workspace-scoped without changing IDs.
ALTER TABLE journal_entries RENAME COLUMN account_id TO workspace_id;
ALTER TABLE brokerage_transactions RENAME COLUMN account_id TO workspace_id;
ALTER TABLE brokerage_holdings RENAME COLUMN account_id TO workspace_id;
ALTER TABLE brokerage_balances RENAME COLUMN account_id TO workspace_id;
ALTER TABLE notebook_folders RENAME COLUMN account_id TO workspace_id;
ALTER TABLE notebook_notes RENAME COLUMN account_id TO workspace_id;
ALTER TABLE notebook_images RENAME COLUMN account_id TO workspace_id;
ALTER TABLE ai_jobs RENAME COLUMN account_id TO workspace_id;
ALTER TABLE ai_source_documents RENAME COLUMN account_id TO workspace_id;
ALTER TABLE ai_artifacts RENAME COLUMN account_id TO workspace_id;
ALTER TABLE user_agents RENAME COLUMN account_id TO workspace_id;
ALTER TABLE position_calculator_rules RENAME COLUMN account_id TO workspace_id;
ALTER TABLE trading_principles RENAME COLUMN account_id TO workspace_id;
ALTER TABLE account_equity_history RENAME COLUMN account_id TO workspace_id;
ALTER TABLE account_equity_rebuild RENAME COLUMN account_id TO workspace_id;
ALTER TABLE brokerage_sync_state RENAME COLUMN account_id TO workspace_id;

-- Data that used to be user-global now belongs to a workspace. Existing rows
-- are assigned to the user's oldest workspace, preserving the previous default.
ALTER TABLE playbooks ADD COLUMN workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE;
ALTER TABLE position_calculator_history ADD COLUMN workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE;
ALTER TABLE position_calculator_plans ADD COLUMN workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE;
ALTER TABLE tag_categories ADD COLUMN workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE;
ALTER TABLE tags ADD COLUMN workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE;

-- Playbooks used to be user-global. Put each one in a workspace where it was
-- actually used, then clone it when the same legacy playbook was used by more
-- than one workspace. Journal and principle links are repointed to the clone.
UPDATE playbooks p SET workspace_id = COALESCE(
    (
        SELECT usage.workspace_id
        FROM (
            SELECT playbook_id, workspace_id FROM journal_entries WHERE playbook_id IS NOT NULL
            UNION
            SELECT playbook_id, workspace_id FROM trading_principles WHERE playbook_id IS NOT NULL
        ) usage
        WHERE usage.playbook_id = p.id
        ORDER BY usage.workspace_id
        LIMIT 1
    ),
    (SELECT id FROM workspaces WHERE user_id = p.user_id ORDER BY created_at, id LIMIT 1)
) WHERE p.workspace_id IS NULL;

CREATE TEMP TABLE workspace_playbook_clones ON COMMIT DROP AS
SELECT DISTINCT
    p.id AS old_id,
    usage.workspace_id,
    gen_random_uuid()::text AS new_id
FROM playbooks p
JOIN (
    SELECT playbook_id, workspace_id FROM journal_entries WHERE playbook_id IS NOT NULL
    UNION
    SELECT playbook_id, workspace_id FROM trading_principles WHERE playbook_id IS NOT NULL
) usage ON usage.playbook_id = p.id
WHERE usage.workspace_id <> p.workspace_id;

INSERT INTO playbooks (
    id, user_id, name, edge_name, entry_rules, exit_rules,
    position_sizing_rules, additional_rules, created_at, updated_at,
    deleted_at, hlc, workspace_id
)
SELECT
    clones.new_id, p.user_id, p.name, p.edge_name, p.entry_rules, p.exit_rules,
    p.position_sizing_rules, p.additional_rules, p.created_at, p.updated_at,
    p.deleted_at, p.hlc, clones.workspace_id
FROM workspace_playbook_clones clones
JOIN playbooks p ON p.id = clones.old_id;

UPDATE journal_entries j SET playbook_id = clones.new_id
FROM workspace_playbook_clones clones
WHERE j.playbook_id = clones.old_id AND j.workspace_id = clones.workspace_id;

UPDATE trading_principles p SET playbook_id = clones.new_id
FROM workspace_playbook_clones clones
WHERE p.playbook_id = clones.old_id AND p.workspace_id = clones.workspace_id;

UPDATE playbooks p SET workspace_id = (
    SELECT id FROM workspaces WHERE user_id = p.user_id ORDER BY created_at, id LIMIT 1
) WHERE p.workspace_id IS NULL;
UPDATE position_calculator_history p SET workspace_id = (
    SELECT id FROM workspaces WHERE user_id = p.user_id ORDER BY created_at, id LIMIT 1
) WHERE p.workspace_id IS NULL;
UPDATE position_calculator_plans p SET workspace_id = (
    SELECT id FROM workspaces WHERE user_id = p.user_id ORDER BY created_at, id LIMIT 1
) WHERE p.workspace_id IS NULL;
UPDATE tag_categories t SET workspace_id = (
    SELECT id FROM workspaces WHERE user_id = t.user_id ORDER BY created_at, id LIMIT 1
) WHERE t.workspace_id IS NULL;
UPDATE tags t SET workspace_id = c.workspace_id
FROM tag_categories c WHERE t.category_id = c.id AND t.workspace_id IS NULL;

ALTER TABLE playbooks ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE position_calculator_history ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE position_calculator_plans ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE tag_categories ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE tags ALTER COLUMN workspace_id SET NOT NULL;

-- The pair prevents a caller from attaching their row to another user's
-- workspace, even if they somehow learn that workspace ID.
ALTER TABLE playbooks ADD CONSTRAINT playbooks_workspace_owner_fk
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE;
ALTER TABLE position_calculator_history ADD CONSTRAINT position_history_workspace_owner_fk
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE;
ALTER TABLE position_calculator_plans ADD CONSTRAINT position_plans_workspace_owner_fk
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE;
ALTER TABLE tag_categories ADD CONSTRAINT tag_categories_workspace_owner_fk
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE;
ALTER TABLE tags ADD CONSTRAINT tags_workspace_owner_fk
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspaces(id, user_id) ON DELETE CASCADE;

CREATE UNIQUE INDEX idx_tag_categories_id_workspace
    ON tag_categories (id, workspace_id);
ALTER TABLE tags ADD CONSTRAINT tags_category_workspace_fk
    FOREIGN KEY (category_id, workspace_id)
    REFERENCES tag_categories(id, workspace_id) ON DELETE CASCADE;

CREATE UNIQUE INDEX idx_playbooks_id_workspace ON playbooks (id, workspace_id);
ALTER TABLE journal_entries ADD CONSTRAINT journal_playbook_workspace_fk
    FOREIGN KEY (playbook_id, workspace_id) REFERENCES playbooks(id, workspace_id);
ALTER TABLE trading_principles ADD CONSTRAINT principles_playbook_workspace_fk
    FOREIGN KEY (playbook_id, workspace_id) REFERENCES playbooks(id, workspace_id);

DROP INDEX IF EXISTS idx_tagcat_user_name;
DROP INDEX IF EXISTS idx_tagcat_user_role;
DROP INDEX IF EXISTS idx_tags_user_cat_name;
CREATE UNIQUE INDEX idx_tagcat_workspace_name
    ON tag_categories (workspace_id, lower(name)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX idx_tagcat_workspace_role
    ON tag_categories (workspace_id, role) WHERE role IS NOT NULL AND deleted_at IS NULL;
CREATE UNIQUE INDEX idx_tags_workspace_cat_name
    ON tags (workspace_id, category_id, lower(name)) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_playbooks_workspace ON playbooks (workspace_id);
CREATE INDEX IF NOT EXISTS idx_position_history_workspace ON position_calculator_history (workspace_id);
CREATE INDEX IF NOT EXISTS idx_position_plans_workspace ON position_calculator_plans (workspace_id);
CREATE INDEX IF NOT EXISTS idx_tags_workspace ON tags (workspace_id);

-- Keep object names aligned with the new domain language where practical.
ALTER INDEX IF EXISTS idx_accounts_user_id RENAME TO idx_workspaces_user_id;
ALTER INDEX IF EXISTS idx_brokerage_tx_user_account RENAME TO idx_brokerage_tx_user_workspace;
ALTER INDEX IF EXISTS idx_brokerage_holdings_user_account RENAME TO idx_brokerage_holdings_user_workspace;
ALTER INDEX IF EXISTS idx_brokerage_balances_user_account RENAME TO idx_brokerage_balances_user_workspace;
ALTER INDEX IF EXISTS idx_notebook_folders_account_id RENAME TO idx_notebook_folders_workspace_id;
ALTER INDEX IF EXISTS idx_notebook_notes_account_id RENAME TO idx_notebook_notes_workspace_id;
ALTER INDEX IF EXISTS idx_notebook_images_account_id RENAME TO idx_notebook_images_workspace_id;
ALTER INDEX IF EXISTS idx_user_agents_account RENAME TO idx_user_agents_workspace;
ALTER INDEX IF EXISTS idx_aeh_account_date RENAME TO idx_aeh_workspace_date;
