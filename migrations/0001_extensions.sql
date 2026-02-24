-- 0001: Enable required PostgreSQL extensions
-- Extensions are also pre-created via docker-entrypoint-initdb.d for fresh installs,
-- but this migration ensures they exist when running sqlx migrate against an existing DB.

CREATE EXTENSION IF NOT EXISTS ltree;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- pgvector: available if using pgvector/pgvector image; Phase 2+ will add VECTOR columns.
-- CREATE EXTENSION IF NOT EXISTS vector;
