-- 0010: Refresh token family tracking for reuse detection.
-- Each token chain from a single login belongs to the same family_id.
-- When rotation occurs, the old token is marked revoked (revoked_at).
-- If a revoked token is presented again, the entire family is purged.

ALTER TABLE refresh_tokens
    ADD COLUMN family_id  UUID NOT NULL DEFAULT uuid_generate_v4(),
    ADD COLUMN revoked_at TIMESTAMPTZ;

CREATE INDEX idx_refresh_tokens_family_id ON refresh_tokens (family_id);
