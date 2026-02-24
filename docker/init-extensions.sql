-- Pre-create extensions so they're available before sqlx migrations run.
-- The pgvector/pgvector:pg16 image already bundles pgvector.
CREATE EXTENSION IF NOT EXISTS ltree;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS vector;
-- PostGIS would require a different base image; add when needed in Phase 2.
