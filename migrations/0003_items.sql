-- 0003: Unified items table (read projection) and deferred FK on users.container_id

CREATE TABLE items (
    id                UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    system_barcode    VARCHAR(32) UNIQUE NOT NULL,
    node_id           VARCHAR(16) UNIQUE NOT NULL,

    -- Classification
    name              VARCHAR(512),
    description       TEXT,
    category          VARCHAR(128),
    tags              TEXT[] DEFAULT '{}',

    -- Hierarchy
    is_container      BOOLEAN NOT NULL DEFAULT FALSE,
    container_path    LTREE,
    parent_id         UUID REFERENCES items(id),

    -- Polymorphic coordinate within parent
    coordinate        JSONB,

    -- Container properties
    location_schema   JSONB,
    max_capacity_cc   NUMERIC,
    max_weight_grams  NUMERIC,

    -- Physical properties
    dimensions        JSONB,
    weight_grams      NUMERIC,

    -- Fungible commodity tracking
    is_fungible       BOOLEAN NOT NULL DEFAULT FALSE,
    fungible_quantity INTEGER,
    fungible_unit     VARCHAR(32),

    -- External identifiers
    external_codes    JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Condition & valuation
    condition         VARCHAR(32) CHECK (condition IS NULL OR condition IN (
                          'new','like_new','good','fair','poor','broken')),
    acquisition_date  DATE,
    acquisition_cost  NUMERIC(12,2),
    current_value     NUMERIC(12,2),
    depreciation_rate NUMERIC(5,4),
    warranty_expiry   DATE,

    -- Extensible metadata
    metadata          JSONB NOT NULL DEFAULT '{}'::jsonb,
    images            JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Full-text search
    search_vector     TSVECTOR,

    -- Audit
    is_deleted        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by        UUID REFERENCES users(id),
    updated_by        UUID REFERENCES users(id)
);

-- ============================
-- Indexes
-- ============================

-- Hierarchy: fast subtree queries
CREATE INDEX idx_items_container_path_gist ON items USING GIST (container_path);
CREATE INDEX idx_items_parent_id           ON items (parent_id);

-- Search: full-text
CREATE INDEX idx_items_search_vector_gin ON items USING GIN (search_vector);

-- Search: fuzzy trigram
CREATE INDEX idx_items_name_trgm ON items USING GIN (name gin_trgm_ops);

-- JSONB: external codes, metadata
CREATE INDEX idx_items_external_codes_gin ON items USING GIN (external_codes jsonb_path_ops);
CREATE INDEX idx_items_metadata_gin       ON items USING GIN (metadata jsonb_path_ops);

-- Tags
CREATE INDEX idx_items_tags_gin ON items USING GIN (tags);

-- B-tree lookups
CREATE INDEX idx_items_system_barcode ON items (system_barcode);
CREATE INDEX idx_items_category       ON items (category);
CREATE INDEX idx_items_condition       ON items (condition);

-- Partial indexes for common filters
CREATE INDEX idx_items_is_container ON items (id) WHERE is_container = TRUE;
CREATE INDEX idx_items_active       ON items (id) WHERE is_deleted = FALSE;

-- ============================
-- Full-text search trigger
-- ============================
CREATE OR REPLACE FUNCTION items_search_vector_update() RETURNS trigger AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', COALESCE(NEW.name, '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(NEW.category, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(NEW.description, '')), 'C') ||
        setweight(to_tsvector('english', COALESCE(
            (SELECT string_agg(value::text, ' ')
             FROM jsonb_each_text(NEW.metadata)), '')), 'D');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_items_search_vector
    BEFORE INSERT OR UPDATE OF name, description, category, metadata
    ON items
    FOR EACH ROW
    EXECUTE FUNCTION items_search_vector_update();

-- ============================
-- updated_at auto-trigger
-- ============================
CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_items_updated_at
    BEFORE UPDATE ON items
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

-- ============================
-- Add deferred FK from users.container_id → items.id
-- ============================
ALTER TABLE users
    ADD CONSTRAINT fk_users_container_id
    FOREIGN KEY (container_id) REFERENCES items(id);

-- ============================
-- Seed: Root container and Users container
-- ============================
INSERT INTO items (id, system_barcode, node_id, name, is_container, container_path, description)
VALUES
    ('00000000-0000-0000-0000-000000000001'::uuid, 'HOM-ROOT', 'n_00000001',
     'Root', TRUE, 'n_00000001', 'Top-level root container'),
    ('00000000-0000-0000-0000-000000000002'::uuid, 'HOM-USERS', 'n_00000002',
     'Users', TRUE, 'n_00000001.n_00000002', 'Ephemeral user containers');

-- Set parent_id for Users → Root
UPDATE items SET parent_id = '00000000-0000-0000-0000-000000000001'::uuid
WHERE id = '00000000-0000-0000-0000-000000000002'::uuid;
