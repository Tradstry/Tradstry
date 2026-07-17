-- Every account gets one System folder. It is the write target for agent-generated notes
-- (a report produced over MCP lands here), so it must exist before anything tries to write
-- into it, and must survive any user action — hence system-owned: no rename, no delete.
ALTER TABLE notebook_folders ADD COLUMN IF NOT EXISTS is_system BOOLEAN NOT NULL DEFAULT false;

-- One per account, enforced in the database rather than trusted to the provisioning code:
-- the ensure path runs on every account creation and on backfill, and a duplicate System
-- folder would be invisible in the UI but split the notes that land in it.
CREATE UNIQUE INDEX IF NOT EXISTS ux_notebook_folders_one_system_per_account
    ON notebook_folders (account_id) WHERE is_system;

-- Backfill: existing accounts predate the folder and would otherwise never get one.
INSERT INTO notebook_folders (id, user_id, account_id, parent_folder_id, name, sort_order, is_system)
SELECT gen_random_uuid()::text, a.user_id, a.id, NULL, 'System', -1, true
FROM accounts a
ON CONFLICT DO NOTHING;
