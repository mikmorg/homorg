-- Migration 0009: Integrity constraints, extensibility columns, and search improvements
--
-- HIGH: schema_version on event_store, search_vector trigger fix, CHECK constraints
-- MEDIUM: deleted_at, currency, pgvector, AI classification, token hash unique,
--         scan session improvements

-- ============================================================
-- 1. Event store: schema_version for forward-compatible event evolution
-- ============================================================
ALTER TABLE event_store ADD COLUMN schema_version INTEGER NOT NULL DEFAULT 1;

-- ============================================================
-- 2. Fix search_vector trigger to include tags
-- ============================================================

-- Replace the function to include tags at weight B (same as category)
CREATE OR REPLACE FUNCTION items_search_vector_update() RETURNS trigger AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', COALESCE(NEW.name, '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(NEW.category, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(array_to_string(NEW.tags, ' '), '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(NEW.description, '')), 'C') ||
        setweight(to_tsvector('english', COALESCE(
            (SELECT string_agg(value::text, ' ')
             FROM jsonb_each_text(NEW.metadata)), '')), 'D');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Re-create trigger to also fire on tags changes
DROP TRIGGER trg_items_search_vector ON items;
CREATE TRIGGER trg_items_search_vector
    BEFORE INSERT OR UPDATE OF name, description, category, metadata, tags
    ON items
    FOR EACH ROW
    EXECUTE FUNCTION items_search_vector_update();

-- Backfill search_vector for existing rows so tags are indexed
UPDATE items SET search_vector =
    setweight(to_tsvector('english', COALESCE(name, '')), 'A') ||
    setweight(to_tsvector('english', COALESCE(category, '')), 'B') ||
    setweight(to_tsvector('english', COALESCE(array_to_string(tags, ' '), '')), 'B') ||
    setweight(to_tsvector('english', COALESCE(description, '')), 'C') ||
    setweight(to_tsvector('english', COALESCE(
        (SELECT string_agg(value::text, ' ')
         FROM jsonb_each_text(metadata)), '')), 'D');

-- ============================================================
-- 3. CHECK constraints on numeric columns (non-negative)
-- ============================================================
ALTER TABLE items ADD CONSTRAINT chk_weight_grams_non_negative
    CHECK (weight_grams IS NULL OR weight_grams >= 0);

ALTER TABLE items ADD CONSTRAINT chk_max_capacity_cc_non_negative
    CHECK (max_capacity_cc IS NULL OR max_capacity_cc >= 0);

ALTER TABLE items ADD CONSTRAINT chk_max_weight_grams_non_negative
    CHECK (max_weight_grams IS NULL OR max_weight_grams >= 0);

ALTER TABLE items ADD CONSTRAINT chk_acquisition_cost_non_negative
    CHECK (acquisition_cost IS NULL OR acquisition_cost >= 0);

ALTER TABLE items ADD CONSTRAINT chk_current_value_non_negative
    CHECK (current_value IS NULL OR current_value >= 0);

ALTER TABLE items ADD CONSTRAINT chk_fungible_quantity_non_negative
    CHECK (fungible_quantity IS NULL OR fungible_quantity >= 0);

-- Fungible consistency: non-fungible items cannot carry a quantity
ALTER TABLE items ADD CONSTRAINT chk_fungible_consistency
    CHECK (is_fungible OR fungible_quantity IS NULL);

-- ============================================================
-- 4. Soft-delete timestamp
-- ============================================================
ALTER TABLE items ADD COLUMN deleted_at TIMESTAMPTZ;

-- ============================================================
-- 5. Currency for monetary values (ISO 4217)
-- ============================================================
ALTER TABLE items ADD COLUMN currency VARCHAR(3)
    CHECK (currency IS NULL OR length(currency) = 3);

-- ============================================================
-- 6. pgvector extension and embedding column
-- ============================================================
CREATE EXTENSION IF NOT EXISTS vector;

ALTER TABLE items ADD COLUMN embedding VECTOR(1536);
-- HNSW index deferred until embedding pipeline is built

-- ============================================================
-- 7. AI classification columns
-- ============================================================
ALTER TABLE items ADD COLUMN classification_confidence REAL;
ALTER TABLE items ADD COLUMN needs_review BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE items ADD COLUMN ai_description TEXT;

-- ============================================================
-- 8. Unique constraint on refresh_tokens.token_hash
-- ============================================================
CREATE UNIQUE INDEX idx_refresh_tokens_hash_unique ON refresh_tokens (token_hash);
-- Drop the old non-unique index (now redundant)
DROP INDEX idx_refresh_tokens_hash;

-- ============================================================
-- 9. Scan session improvements
-- ============================================================
ALTER TABLE scan_sessions ADD COLUMN device_id VARCHAR(128);
ALTER TABLE scan_sessions ADD COLUMN items_errored INTEGER NOT NULL DEFAULT 0;

-- Partial index for active session lookups (ended_at IS NULL)
CREATE INDEX idx_scan_sessions_active ON scan_sessions (user_id) WHERE ended_at IS NULL;
