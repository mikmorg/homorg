-- Migration 0016: Schema cleanup
--
-- Fixes identified during deep analysis audit:
--
-- 1. Drop vestigial `items.container_type_id` column.
--    The canonical source is `container_properties.container_type_id`.
--    All queries already read from the JOIN (`cp.container_type_id`).
--    The projector writes only to container_properties.
--    The column on items was never updated by application code — only the
--    sync trigger set it to NULL on container_properties DELETE.
--
-- 2. Update `sync_item_type_flags()` to stop referencing the dropped column.
--
-- 3. Add a trigger to prevent circular `parent_category_id` references
--    in the categories table.
--
-- NOTE: Migration 0009 added CHECK constraints (`chk_max_capacity_cc_non_negative`,
-- `chk_max_weight_grams_non_negative`, `chk_fungible_quantity_non_negative`,
-- `chk_fungible_consistency`) on items columns that were dropped in migration
-- 0014.  PostgreSQL silently drops CHECK constraints when their columns are
-- dropped, so these are already gone.  No action needed.


-- ═══════════════════════════════════════════════════════════════════════════
-- 1. Update sync trigger BEFORE dropping the column it references
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
            UPDATE items SET is_container = FALSE WHERE id = OLD.item_id;
        ELSIF TG_TABLE_NAME = 'fungible_properties' THEN
            UPDATE items SET is_fungible = FALSE WHERE id = OLD.item_id;
        END IF;
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ═══════════════════════════════════════════════════════════════════════════
-- 2. Drop the vestigial column (FK constraint and any indexes are
--    dropped automatically by PostgreSQL)
-- ═══════════════════════════════════════════════════════════════════════════

ALTER TABLE items DROP COLUMN IF EXISTS container_type_id;

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. Prevent circular category hierarchy references
--    Walking the ancestor chain is bounded to 100 levels (well beyond any
--    reasonable taxonomy depth) to avoid infinite loops on corrupt data.
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION check_category_no_cycle() RETURNS trigger AS $$
DECLARE
    v_current UUID;
    v_depth   INT := 0;
BEGIN
    IF NEW.parent_category_id IS NULL THEN
        RETURN NEW;
    END IF;

    -- Self-reference
    IF NEW.parent_category_id = NEW.id THEN
        RAISE EXCEPTION 'Category cannot be its own parent';
    END IF;

    -- Walk ancestors from the proposed parent; if we reach NEW.id it's a cycle
    v_current := NEW.parent_category_id;
    LOOP
        SELECT parent_category_id INTO v_current
        FROM categories
        WHERE id = v_current;

        IF NOT FOUND OR v_current IS NULL THEN
            EXIT;  -- Reached a root — no cycle
        END IF;

        IF v_current = NEW.id THEN
            RAISE EXCEPTION 'parent_category_id would create a circular reference';
        END IF;

        v_depth := v_depth + 1;
        IF v_depth > 100 THEN
            RAISE EXCEPTION 'Category hierarchy too deep (possible cycle)';
        END IF;
    END LOOP;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_categories_no_cycle
    BEFORE INSERT OR UPDATE OF parent_category_id ON categories
    FOR EACH ROW
    EXECUTE FUNCTION check_category_no_cycle();
