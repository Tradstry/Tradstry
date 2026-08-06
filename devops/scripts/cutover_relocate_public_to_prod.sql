-- Cutover step 1: relocate the EXISTING production tables from `public` into
-- `tradstry_prod`, so the schema-partitioned app (POSTGRES_DATABASE=prod) keeps
-- serving the current data instead of starting empty.
--
-- `ALTER ... SET SCHEMA` is metadata-only: it preserves all rows and indexes,
-- no copy. Indexes and table-owned sequences move with their table
-- automatically; standalone sequences are moved explicitly below. Postgres
-- EXTENSIONS (vector, pg_search) intentionally STAY in `public` — the app's
-- search_path is `tradstry_prod, public`, so extension types still resolve.
--
-- Run this ONCE, in a brief maintenance window, BEFORE booting the app with
-- POSTGRES_DATABASE=prod and BEFORE running the prod data transfer. Idempotent:
-- re-running skips objects already moved (they're no longer in public).
--
--   psql "$POSTGRES_URL" -f devops/scripts/cutover_relocate_public_to_prod.sql

BEGIN;

CREATE SCHEMA IF NOT EXISTS tradstry_prod;

DO $$
DECLARE r record;
BEGIN
    -- Tables (indexes + owned sequences follow automatically).
    FOR r IN
        SELECT tablename FROM pg_tables WHERE schemaname = 'public'
    LOOP
        EXECUTE format('ALTER TABLE public.%I SET SCHEMA tradstry_prod', r.tablename);
        RAISE NOTICE 'moved table %', r.tablename;
    END LOOP;

    -- Views.
    FOR r IN
        SELECT table_name FROM information_schema.views WHERE table_schema = 'public'
    LOOP
        EXECUTE format('ALTER VIEW public.%I SET SCHEMA tradstry_prod', r.table_name);
        RAISE NOTICE 'moved view %', r.table_name;
    END LOOP;

    -- Standalone sequences not owned by a moved table.
    FOR r IN
        SELECT sequence_name FROM information_schema.sequences WHERE sequence_schema = 'public'
    LOOP
        EXECUTE format('ALTER SEQUENCE public.%I SET SCHEMA tradstry_prod', r.sequence_name);
        RAISE NOTICE 'moved sequence %', r.sequence_name;
    END LOOP;
END $$;

-- Sanity check: list what now lives in tradstry_prod.
SELECT 'tradstry_prod' AS schema, table_name
FROM information_schema.tables
WHERE table_schema = 'tradstry_prod'
ORDER BY table_name;

COMMIT;
