-- Migration 0021: Remote camera support for stocker sessions
--
-- 1. Adds active_item_id to scan_sessions to track the most recently
--    created/scanned item, enabling auto-attachment of camera photos.
-- 2. Creates camera_tokens table for linking remote camera devices to sessions
--    via short-lived opaque tokens (no full JWT auth required on the camera).

-- ── scan_sessions: active item tracking ─────────────────────────────────
ALTER TABLE scan_sessions ADD COLUMN active_item_id UUID REFERENCES items(id);

COMMENT ON COLUMN scan_sessions.active_item_id IS
    'Most recently created or scanned item in this session. Camera uploads attach to this item.';

-- ── camera_tokens: remote camera device linking ─────────────────────────
CREATE TABLE camera_tokens (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    session_id  UUID NOT NULL REFERENCES scan_sessions(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id),
    token       VARCHAR(64) NOT NULL UNIQUE,
    device_name VARCHAR(128),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked_at  TIMESTAMPTZ
);

CREATE INDEX idx_camera_tokens_token ON camera_tokens (token) WHERE revoked_at IS NULL;
CREATE INDEX idx_camera_tokens_session_id ON camera_tokens (session_id);

COMMENT ON TABLE camera_tokens IS
    'Opaque bearer tokens for remote camera devices. Scoped to a single scan session.';
