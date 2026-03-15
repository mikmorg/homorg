-- Migration 0015: Normalize tags and categories
--
-- Purpose: tags stored as TEXT[] arrays cannot be renamed without scanning and
-- updating every item row.  Similarly, categories as bare VARCHAR strings are
-- hard to enumerate, merge, or rename.  This migration introduces proper
-- reference tables so that renaming a tag or category is a single UPDATE on
-- one row.
--
-- Changes:
--  1. Create tags table
--  2. Create categories table (with optional hierarchy via parent_category_id)
--  3. Create item_tags junction table
--  4. Add items.category_id FK → categories
--  5. Migrate existing TEXT[] tags and VARCHAR category data
--  6. Drop the old columns from items
--  7. Replace the full-text search trigger to read tags/category via subqueries
--     and add a trigger on item_tags so that tag additions/removals refresh
--     the items.search_vector automatically.

-- ═══════════════════════════════════════════════════════════════════════════
-- 1. Reference tables
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE categories (
    id                 UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name               VARCHAR(128) NOT NULL,
    description        TEXT,
    parent_category_id UUID REFERENCES categories(id) ON DELETE SET NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_categories_name UNIQUE (name)
);

CREATE INDEX idx_categories_name  ON categories (name);
CREATE INDEX idx_categories_parent ON categories (parent_category_id);

CREATE TRIGGER trg_categories_updated_at
    BEFORE UPDATE ON categories
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE tags (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name       VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_tags_name UNIQUE (name)
);

CREATE INDEX idx_tags_name ON tags (name);

-- ═══════════════════════════════════════════════════════════════════════════
-- 2. Junction table
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE item_tags (
    item_id UUID NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    tag_id  UUID NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (item_id, tag_id)
);

CREATE INDEX idx_item_tags_tag_id ON item_tags (tag_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. Add category_id to items
-- ═══════════════════════════════════════════════════════════════════════════

ALTER TABLE items ADD COLUMN category_id UUID REFERENCES categories(id) ON DELETE SET NULL;
CREATE INDEX idx_items_category_id ON items (category_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- 4. Data migration: populate normalized tables from existing flat data
-- ═══════════════════════════════════════════════════════════════════════════

-- 4a. Insert distinct category names (excluding NULLs)
INSERT INTO categories (name)
SELECT DISTINCT category
FROM items
WHERE category IS NOT NULL AND category <> ''
ON CONFLICT (name) DO NOTHING;

-- 4b. Backfill items.category_id
UPDATE items i
SET category_id = c.id
FROM categories c
WHERE i.category = c.name;

-- 4c. Insert distinct tag names from all items
INSERT INTO tags (name)
SELECT DISTINCT tag
FROM items, unnest(tags) AS tag
WHERE tag IS NOT NULL AND tag <> ''
ON CONFLICT (name) DO NOTHING;

-- 4d. Populate item_tags junction from existing TEXT[] tags
INSERT INTO item_tags (item_id, tag_id)
SELECT DISTINCT i.id, t.id
FROM items i
JOIN unnest(i.tags) AS tag_name ON TRUE
JOIN tags t ON t.name = tag_name
WHERE tag_name IS NOT NULL AND tag_name <> ''
ON CONFLICT DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- 5. Drop old flat columns
--    Must drop the search trigger first: migration 0009 created it with an
--    UPDATE OF column-filter that references `category` and `tags`.
--    PostgreSQL refuses to drop a column referenced in a trigger's column
--    list, so we remove the trigger here and recreate it in step 6.
-- ═══════════════════════════════════════════════════════════════════════════

DROP TRIGGER IF EXISTS trg_items_search_vector ON items;

ALTER TABLE items DROP COLUMN IF EXISTS category;
ALTER TABLE items DROP COLUMN IF EXISTS tags;

-- Also drop/update old category index (it was on the now-dropped column)
DROP INDEX IF EXISTS idx_items_category;
-- Drop the GIN tags index (column removed)
DROP INDEX IF EXISTS idx_items_tags_gin;

-- ═══════════════════════════════════════════════════════════════════════════
-- 6. Updated full-text search function — reads tags and category via subqueries
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION items_search_vector_update() RETURNS trigger AS $$
DECLARE
    v_category_name TEXT;
    v_tags_text     TEXT;
BEGIN
    -- Resolve category name from FK
    SELECT name INTO v_category_name
    FROM categories
    WHERE id = NEW.category_id;

    -- Aggregate tag names via junction table
    SELECT string_agg(t.name, ' ' ORDER BY t.name)
    INTO v_tags_text
    FROM item_tags it2
    JOIN tags t ON t.id = it2.tag_id
    WHERE it2.item_id = NEW.id;

    NEW.search_vector :=
        setweight(to_tsvector('english', COALESCE(NEW.name, '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(v_category_name, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(v_tags_text, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(NEW.description, '')), 'C') ||
        setweight(to_tsvector('english', COALESCE(
            (SELECT string_agg(value::text, ' ')
             FROM jsonb_each_text(NEW.metadata)), '')), 'D');

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Re-create trigger to also fire on category_id changes
DROP TRIGGER IF EXISTS trg_items_search_vector ON items;
CREATE TRIGGER trg_items_search_vector
    BEFORE INSERT OR UPDATE OF name, description, category_id, metadata
    ON items
    FOR EACH ROW
    EXECUTE FUNCTION items_search_vector_update();

-- ═══════════════════════════════════════════════════════════════════════════
-- 7. Trigger on item_tags to keep search_vector current when tags change
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION refresh_item_search_vector_on_tag_change() RETURNS trigger AS $$
DECLARE
    v_item_id UUID;
BEGIN
    -- Determine which item was affected
    v_item_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.item_id ELSE NEW.item_id END;

    -- Force items row trigger by touching a non-indexed column
    UPDATE items SET search_vector = items_search_vector_update_for(id)
    WHERE id = v_item_id;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Helper function that computes search_vector for a given item_id (used by the tag trigger)
CREATE OR REPLACE FUNCTION items_search_vector_update_for(p_item_id UUID) RETURNS tsvector AS $$
DECLARE
    v_item     items%ROWTYPE;
    v_category TEXT;
    v_tags     TEXT;
BEGIN
    SELECT * INTO v_item FROM items WHERE id = p_item_id;
    IF NOT FOUND THEN RETURN to_tsvector(''); END IF;

    SELECT name INTO v_category FROM categories WHERE id = v_item.category_id;

    SELECT string_agg(t.name, ' ' ORDER BY t.name)
    INTO v_tags
    FROM item_tags it2
    JOIN tags t ON t.id = it2.tag_id
    WHERE it2.item_id = p_item_id;

    RETURN
        setweight(to_tsvector('english', COALESCE(v_item.name, '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(v_category, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(v_tags, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(v_item.description, '')), 'C') ||
        setweight(to_tsvector('english', COALESCE(
            (SELECT string_agg(value::text, ' ') FROM jsonb_each_text(v_item.metadata)), '')), 'D');
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_item_tags_refresh_search
    AFTER INSERT OR DELETE ON item_tags
    FOR EACH ROW
    EXECUTE FUNCTION refresh_item_search_vector_on_tag_change();

-- ═══════════════════════════════════════════════════════════════════════════
-- 8. Rebuild search_vector for all existing items now that tags/category
--    are stored in normalized tables
-- ═══════════════════════════════════════════════════════════════════════════

UPDATE items SET search_vector = items_search_vector_update_for(id);

