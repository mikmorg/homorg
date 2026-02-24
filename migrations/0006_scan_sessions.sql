-- 0006: Scan sessions for stocker workflow tracking

CREATE TABLE scan_sessions (
    id                  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id             UUID NOT NULL REFERENCES users(id),
    active_container_id UUID REFERENCES items(id),
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at            TIMESTAMPTZ,
    items_scanned       INTEGER NOT NULL DEFAULT 0,
    items_created       INTEGER NOT NULL DEFAULT 0,
    items_moved         INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_scan_sessions_user_id ON scan_sessions (user_id);
CREATE INDEX idx_scan_sessions_started_at ON scan_sessions (started_at DESC);
