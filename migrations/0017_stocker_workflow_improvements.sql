-- Migration 0017: Stocker workflow improvements
--
-- 1. Adds purpose field to container_types for semantic designation (outbox, storage,
--    transit, workspace, etc.).  Free-text, not an enum — preserves flexibility.
-- 2. Adds notes column to scan_sessions for session-level annotations.

-- ── container_types: purpose field ──────────────────────────────────────
ALTER TABLE container_types ADD COLUMN purpose VARCHAR(64);

CREATE INDEX idx_container_types_purpose ON container_types (purpose)
    WHERE purpose IS NOT NULL;

COMMENT ON COLUMN container_types.purpose IS
    'Semantic designation: outbox, storage, transit, workspace, etc. Free-text, not an enum.';

-- ── scan_sessions: notes field ──────────────────────────────────────────
ALTER TABLE scan_sessions ADD COLUMN notes TEXT;
