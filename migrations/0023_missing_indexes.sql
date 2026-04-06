-- Foreign key columns that were missing indexes.
-- These improve query performance for audit lookups and active-session queries.

-- Items: who created / last updated each non-deleted item
CREATE INDEX IF NOT EXISTS idx_items_created_by
    ON items(created_by) WHERE is_deleted = FALSE;

CREATE INDEX IF NOT EXISTS idx_items_updated_by
    ON items(updated_by) WHERE is_deleted = FALSE;

-- Scan sessions: active container / item references (nullable FKs)
CREATE INDEX IF NOT EXISTS idx_scan_sessions_active_container
    ON scan_sessions(active_container_id) WHERE ended_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_scan_sessions_active_item
    ON scan_sessions(active_item_id) WHERE ended_at IS NULL;

-- Camera tokens: look up by user (e.g. revoke all tokens for a user)
CREATE INDEX IF NOT EXISTS idx_camera_tokens_user_id
    ON camera_tokens(user_id);
