-- Migration 0013: Container types
--
-- Purpose: introduce a container_types reference table so that physical storage
-- containers which come in standard sizes / configurations (bins, boxes, shelves)
-- can be given a named type that pre-populates dimensions, weight limits and the
-- internal location schema.  Types are trivial to create inline when making a new
-- container from the UX (one extra field), and container instances reference them
-- as a soft FK so renaming or deleting a type is always safe.
--
-- A container type may be used by a single container (unique to it) or shared across
-- many containers of the same physical form.  Creating a "singleton" type is zero
-- friction and is the expected default.

-- ── container_types ─────────────────────────────────────────────────────────
CREATE TABLE container_types (
    id                       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name                     VARCHAR(128) NOT NULL,
    description              TEXT,
    default_max_capacity_cc  NUMERIC,
    default_max_weight_grams NUMERIC,
    default_dimensions       JSONB,         -- {width_cm, height_cm, depth_cm}
    default_location_schema  JSONB,         -- coordinate system template
    icon                     VARCHAR(64),   -- optional UI icon identifier
    created_by               UUID REFERENCES users(id),
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT chk_ct_max_capacity       CHECK (default_max_capacity_cc IS NULL OR default_max_capacity_cc >= 0),
    CONSTRAINT chk_ct_max_weight         CHECK (default_max_weight_grams IS NULL OR default_max_weight_grams >= 0),
    -- Names are unique per user to allow each user their own type vocabulary
    UNIQUE (name, created_by)
);

CREATE INDEX idx_container_types_name ON container_types (name);
CREATE INDEX idx_container_types_created_by ON container_types (created_by);

-- Reuse set_updated_at() from migration 0003
CREATE TRIGGER trg_container_types_updated_at
    BEFORE UPDATE ON container_types
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

-- ── Add container_type_id reference to items ─────────────────────────────────
-- This will be moved to container_properties in migration 0014; we add it here
-- so it is available from the moment container types are introduced.  The column
-- is nullable and has no functional effect until migration 0014 populates
-- container_properties rows.
ALTER TABLE items ADD COLUMN container_type_id UUID REFERENCES container_types(id) ON DELETE SET NULL;

