-- Migration 0011: Performance indexes, FK integrity, and barcode sequence seeding
-- Addresses: ES-5, DB-3, DB-4, DI-4

-- ─── ES-5: Index for causation_id lookup ────────────────────────────────────
-- `has_compensating_event_in_tx` queries metadata->>'causation_id'.
-- CONCURRENTLY is fine here because SQLx runs migrations in a transaction, but
-- CREATE INDEX CONCURRENTLY cannot run inside a transaction.  Using the
-- non-concurrent form which is safe for fresh deployments and bounded migrations.
CREATE INDEX IF NOT EXISTS idx_event_store_causation_id
    ON event_store ((metadata->>'causation_id'))
    WHERE metadata->>'causation_id' IS NOT NULL;

-- ─── DB-3: Index on current_value for range/filter queries ──────────────────
CREATE INDEX IF NOT EXISTS idx_items_current_value
    ON items (current_value)
    WHERE current_value IS NOT NULL AND is_deleted = FALSE;

-- ─── DB-4: Make users.container_id FK deferrable ────────────────────────────
-- Removing and re-adding the FK with DEFERRABLE INITIALLY DEFERRED allows
-- the setup transaction (that creates the root container + user in one go)
-- to reference the container before it is fully visible.
ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_container_id_fkey;

ALTER TABLE users
    ADD CONSTRAINT users_container_id_fkey
        FOREIGN KEY (container_id)
        REFERENCES items(id)
        DEFERRABLE INITIALLY DEFERRED;

-- ─── DI-4: Seed the default barcode prefix ──────────────────────────────────
-- Ensures there is always a starting sequence row for the default "HOM" prefix
-- so barcode generation does not fail on fresh databases.
INSERT INTO barcode_sequences (prefix, next_value)
VALUES ('HOM', 1)
ON CONFLICT (prefix) DO NOTHING;
