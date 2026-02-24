-- Migration 0008: Schema improvements
--
-- HIGH: Enforce event_id global uniqueness
-- HIGH: Add keyset-pagination index for items listing
-- MEDIUM: Drop redundant index on system_barcode (UNIQUE constraint already creates one)
-- MEDIUM: Replace parent_id index with partial index filtering deleted items
-- MEDIUM: Add updated_at trigger on users table (reuse existing function)
-- LOW: Drop low-value partial indexes on items(id)

-- 1. UNIQUE index on event_store.event_id (idempotent dedup guard)
CREATE UNIQUE INDEX idx_event_store_event_id ON event_store (event_id);

-- 2. Keyset pagination index for items listing
CREATE INDEX idx_items_created_at_id ON items (created_at, id) WHERE is_deleted = FALSE;

-- 3. Drop redundant index — the UNIQUE constraint on system_barcode already creates an implicit index
DROP INDEX idx_items_system_barcode;

-- 4. Replace blanket parent_id index with partial index (dead rows excluded)
DROP INDEX idx_items_parent_id;
CREATE INDEX idx_items_parent_active ON items (parent_id) WHERE is_deleted = FALSE;

-- 5. Automatic updated_at trigger on users (reuses set_updated_at() from migration 0003)
CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

-- 6. Drop low-value partial indexes that scan on items(id) — primary key already covers lookups
DROP INDEX idx_items_is_container;
DROP INDEX idx_items_active;
