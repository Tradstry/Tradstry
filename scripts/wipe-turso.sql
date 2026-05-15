-- Once a while i like to wipe dev database where i test stuff for local development so i use this 

-- Wipe every Tradstry table in the Turso/libSQL database.
-- Keeps the schema intact (no DROP TABLE).
--
-- DELETEs are ordered child → parent so foreign-key constraints don't fire.
-- Safe to paste into the Turso/Drizzle web console (which auto-wraps in a
-- transaction — no BEGIN/COMMIT needed).
--
-- Usage:
--   - Turso web console / Drizzle Studio: paste and run
--   - CLI:  turso db shell <your-db-name> < wipe-turso.sql
--   - SQLite: sqlite3 your-local.db < wipe-turso.sql

DELETE FROM journal_brokerage_links;
DELETE FROM notebook_note_trades;
DELETE FROM notebook_images;
DELETE FROM notebook_notes;
DELETE FROM ai_artifact_sources;
DELETE FROM ai_artifacts;
DELETE FROM ai_source_documents;
DELETE FROM ai_jobs;
DELETE FROM chat_sessions;
DELETE FROM brokerage_balances;
DELETE FROM brokerage_holdings;
DELETE FROM brokerage_transactions;
DELETE FROM journal_entries;
DELETE FROM playbooks;
DELETE FROM position_calculator_history;
DELETE FROM position_calculator_plans;
DELETE FROM position_calculator_rules;
DELETE FROM user_agents;
DELETE FROM user_prompts;
DELETE FROM accounts;
DELETE FROM users;

-- Verify (every count should be 0):
SELECT 'users' AS table_name, COUNT(*) AS rows FROM users
UNION ALL SELECT 'accounts', COUNT(*) FROM accounts
UNION ALL SELECT 'journal_entries', COUNT(*) FROM journal_entries
UNION ALL SELECT 'journal_brokerage_links', COUNT(*) FROM journal_brokerage_links
UNION ALL SELECT 'playbooks', COUNT(*) FROM playbooks
UNION ALL SELECT 'brokerage_transactions', COUNT(*) FROM brokerage_transactions
UNION ALL SELECT 'brokerage_holdings', COUNT(*) FROM brokerage_holdings
UNION ALL SELECT 'brokerage_balances', COUNT(*) FROM brokerage_balances
UNION ALL SELECT 'notebook_notes', COUNT(*) FROM notebook_notes
UNION ALL SELECT 'notebook_images', COUNT(*) FROM notebook_images
UNION ALL SELECT 'notebook_note_trades', COUNT(*) FROM notebook_note_trades
UNION ALL SELECT 'chat_sessions', COUNT(*) FROM chat_sessions
UNION ALL SELECT 'ai_jobs', COUNT(*) FROM ai_jobs
UNION ALL SELECT 'ai_artifacts', COUNT(*) FROM ai_artifacts
UNION ALL SELECT 'ai_artifact_sources', COUNT(*) FROM ai_artifact_sources
UNION ALL SELECT 'ai_source_documents', COUNT(*) FROM ai_source_documents
UNION ALL SELECT 'user_agents', COUNT(*) FROM user_agents
UNION ALL SELECT 'user_prompts', COUNT(*) FROM user_prompts
UNION ALL SELECT 'position_calculator_history', COUNT(*) FROM position_calculator_history
UNION ALL SELECT 'position_calculator_plans', COUNT(*) FROM position_calculator_plans
UNION ALL SELECT 'position_calculator_rules', COUNT(*) FROM position_calculator_rules;
