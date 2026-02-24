-- 0002: Users table
-- container_id FK to items is deferred (ALTER TABLE after items table exists in 0003).

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username        VARCHAR(64)  UNIQUE NOT NULL,
    password_hash   VARCHAR(256) NOT NULL,
    display_name    VARCHAR(128),
    role            VARCHAR(16)  NOT NULL DEFAULT 'member'
                        CHECK (role IN ('admin', 'member', 'readonly')),
    container_id    UUID,  -- FK added after items table exists
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Invite tokens for household member registration
CREATE TABLE invite_tokens (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    code        VARCHAR(64) UNIQUE NOT NULL,
    created_by  UUID NOT NULL REFERENCES users(id),
    used_by     UUID REFERENCES users(id),
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_invite_tokens_code ON invite_tokens(code) WHERE used_by IS NULL;
