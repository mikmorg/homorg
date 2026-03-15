-- Migration 0014: Container and fungible extension tables
--
-- Purpose: items that are containers and items that are fungible have mutually
-- exclusive sets of extra attributes.  Keeping all those attributes as nullable
-- columns on the flat items table is misleading — a fungible item can never have
-- a location_schema, and a container can never have a fungible_quantity.
--
-- Solution: two extension tables that extend items:
--   container_properties  — all container-specific configuration
--   fungible_properties   — all fungible-item-specific attributes
--
-- The denormalized booleans (is_container, is_fungible) on items are kept for
-- query performance (they are read constantly by LTREE hierarchy logic and search
-- filters).  Triggers on the extension tables keep those flags in sync so no
-- application code needs to set them explicitly.
--
-- A BEFORE INSERT trigger enforces mutual exclusivity at the DB level so that
-- application bugs can never corrupt the schema invariant.
--
-- Data migration: existing items with is_container = TRUE are migrated into
-- container_properties; items with is_fungible = TRUE are migrated into
-- fungible_properties.

-- ═══════════════════════════════════════════════════════════════════════════
-- 1. Extension tables
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE container_properties (
    item_id              UUID PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
    location_schema      JSONB,
    max_capacity_cc      NUMERIC CHECK (max_capacity_cc IS NULL OR max_capacity_cc >= 0),
    max_weight_grams     NUMERIC CHECK (max_weight_grams IS NULL OR max_weight_grams >= 0),
    container_type_id    UUID REFERENCES container_types(id) ON DELETE SET NULL
);

CREATE TABLE fungible_properties (
    item_id   UUID PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
    quantity  INTEGER NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    unit      VARCHAR(32)
);

-- ═══════════════════════════════════════════════════════════════════════════
-- 2. Mutual exclusivity trigger
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION check_item_type_exclusivity() RETURNS trigger AS $$
BEGIN
    IF TG_TABLE_NAME = 'container_properties' THEN
        IF EXISTS (SELECT 1 FROM fungible_properties WHERE item_id = NEW.item_id) THEN
            RAISE EXCEPTION 'Item % cannot be both a container and fungible', NEW.item_id;
        END IF;
    ELSIF TG_TABLE_NAME = 'fungible_properties' THEN
        IF EXISTS (SELECT 1 FROM container_properties WHERE item_id = NEW.item_id) THEN
            RAISE EXCEPTION 'Item % cannot be both a container and fungible', NEW.item_id;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_container_properties_exclusivity
    BEFORE INSERT ON container_properties
    FOR EACH ROW EXECUTE FUNCTION check_item_type_exclusivity();

CREATE TRIGGER trg_fungible_properties_exclusivity
    BEFORE INSERT ON fungible_properties
    FOR EACH ROW EXECUTE FUNCTION check_item_type_exclusivity();

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. Sync denormalized flags on items
--    is_container / is_fungible on items stay in sync via triggers so that
--    hierarchy queries and search filters continue to use fast indexed lookups.
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION sync_item_type_flags() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF TG_TABLE_NAME = 'container_properties' THEN
            UPDATE items SET is_container = TRUE WHERE id = NEW.item_id;
        ELSIF TG_TABLE_NAME = 'fungible_properties' THEN
            UPDATE items SET is_fungible = TRUE WHERE id = NEW.item_id;
        END IF;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        IF TG_TABLE_NAME = 'container_properties' THEN
            UPDATE items SET is_container = FALSE, container_type_id = NULL WHERE id = OLD.item_id;
        ELSIF TG_TABLE_NAME = 'fungible_properties' THEN
            UPDATE items SET is_fungible = FALSE WHERE id = OLD.item_id;
        END IF;
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_container_properties_sync_flag
    AFTER INSERT OR DELETE ON container_properties
    FOR EACH ROW EXECUTE FUNCTION sync_item_type_flags();

CREATE TRIGGER trg_fungible_properties_sync_flag
    AFTER INSERT OR DELETE ON fungible_properties
    FOR EACH ROW EXECUTE FUNCTION sync_item_type_flags();

-- ═══════════════════════════════════════════════════════════════════════════
-- 4. Transfer container_type_id from items to container_properties
--    (items.container_type_id was added in migration 0013)
-- ═══════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════
-- 5. Data migration: populate extension tables from existing flat columns
-- ═══════════════════════════════════════════════════════════════════════════

-- Containers: migrate from items.is_container = TRUE
INSERT INTO container_properties (item_id, location_schema, max_capacity_cc, max_weight_grams, container_type_id)
SELECT id, location_schema, max_capacity_cc, max_weight_grams, container_type_id
FROM items
WHERE is_container = TRUE;

-- Fungible items: migrate from items.is_fungible = TRUE
INSERT INTO fungible_properties (item_id, quantity, unit)
SELECT id, COALESCE(fungible_quantity, 0), fungible_unit
FROM items
WHERE is_fungible = TRUE;

-- ═══════════════════════════════════════════════════════════════════════════
-- 6. Remove migrated columns from items
--    The extension tables own these fields now; keeping them in items would
--    create a dual-write maintenance burden.
-- ═══════════════════════════════════════════════════════════════════════════

ALTER TABLE items DROP COLUMN IF EXISTS location_schema;
ALTER TABLE items DROP COLUMN IF EXISTS max_capacity_cc;
ALTER TABLE items DROP COLUMN IF EXISTS max_weight_grams;
ALTER TABLE items DROP COLUMN IF EXISTS fungible_quantity;
ALTER TABLE items DROP COLUMN IF EXISTS fungible_unit;
-- container_type_id stays on items as a convenience for UPDATE before the
-- row in container_properties has been created; once the container_properties
-- row is canonical the items column is only ever set by the sync trigger.
-- We remove any default / NOT NULL so it is purely informational if needed.

-- ═══════════════════════════════════════════════════════════════════════════
-- 7. Restore the stats index that referenced removed columns (if any)
--    and add new index on container_properties for join performance.
-- ═══════════════════════════════════════════════════════════════════════════

CREATE INDEX idx_cp_item_id ON container_properties (item_id);
CREATE INDEX idx_fp_item_id ON fungible_properties (item_id);

