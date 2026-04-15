-- 0025: AI enrichment pipeline — system user + task queue + review index
--
-- The enricher daemon (src/bin/enricher) picks up tasks from enrichment_tasks,
-- calls a pluggable EnrichmentProvider (v1: claude CLI), and writes back via
-- ItemUpdated events under the ai-enricher system user's actor_id.
--
-- Low-confidence results flip items.needs_review=TRUE so they surface in the
-- admin review queue (ordered by confidence ascending, age ascending).

-- ── System user for AI-emitted events ──────────────────────────────────────
-- Hardcoded UUID so daemon code can reference it without a lookup.
-- Unusable password_hash ('!') and is_active=FALSE keep the account from being
-- used for real authentication.
INSERT INTO users (id, username, password_hash, display_name, role, is_active)
VALUES (
    '00000000-0000-0000-0000-00000000a1e1',
    'ai-enricher',
    '!',
    'AI Enricher',
    'admin',
    FALSE
)
ON CONFLICT (id) DO NOTHING;

COMMENT ON COLUMN users.password_hash IS
    'Argon2 hash of the user''s password. The sentinel value ''!'' marks a disabled system account that cannot log in (combined with is_active=FALSE).';

-- ── Enrichment task queue ──────────────────────────────────────────────────
CREATE TYPE enrichment_status AS ENUM (
    'pending', 'in_progress', 'succeeded', 'failed', 'dead', 'canceled'
);

CREATE TABLE enrichment_tasks (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    item_id         UUID NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    trigger_event   VARCHAR(64) NOT NULL,
    priority        INTEGER NOT NULL DEFAULT 100,
    status          enrichment_status NOT NULL DEFAULT 'pending',
    attempts        INTEGER NOT NULL DEFAULT 0,
    max_attempts    INTEGER NOT NULL DEFAULT 3,
    provider        VARCHAR(64),
    last_error      TEXT,
    result_summary  JSONB,
    claimed_at      TIMESTAMPTZ,
    claimed_by      VARCHAR(128),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    CONSTRAINT chk_enrichment_attempts_non_negative   CHECK (attempts   >= 0),
    CONSTRAINT chk_enrichment_max_attempts_positive   CHECK (max_attempts > 0),
    CONSTRAINT chk_enrichment_priority_non_negative   CHECK (priority   >= 0)
);

COMMENT ON TABLE enrichment_tasks IS
    'AI enrichment job queue. Daemon claims tasks with SELECT FOR UPDATE SKIP LOCKED ordered by (priority ASC, created_at ASC).';

COMMENT ON COLUMN enrichment_tasks.trigger_event IS
    'Why the task was enqueued: image_added, external_code_added, manual_rerun, follow_up.';

COMMENT ON COLUMN enrichment_tasks.priority IS
    'Lower value = higher priority. Defaults: image_added=100, external_code_added=50 (ISBN is fast/authoritative), manual_rerun=25.';

COMMENT ON COLUMN enrichment_tasks.claimed_by IS
    'Identifier of the daemon that claimed this task, typically "<hostname>:<pid>". Used for orphan recovery on restart.';

COMMENT ON COLUMN enrichment_tasks.result_summary IS
    'JSONB capturing {confidence, model, fields_changed, cost_usd, token_usage, reasoning}. Set on success.';

-- One active task per item at a time. New triggers coalesce into the active
-- row via ON CONFLICT (item_id) WHERE status IN ('pending','in_progress')
-- DO UPDATE SET trigger_event=EXCLUDED.trigger_event, updated_at=NOW().
--
-- When a task completes (succeeded/failed/dead/canceled) the row leaves the
-- partial index, so a follow-up trigger creates a fresh task.
CREATE UNIQUE INDEX idx_enrichment_one_active_per_item
    ON enrichment_tasks(item_id)
    WHERE status IN ('pending', 'in_progress');

-- Claim index: daemon picks oldest highest-priority pending task.
CREATE INDEX idx_enrichment_claim
    ON enrichment_tasks(priority, created_at)
    WHERE status = 'pending';

-- Monitoring: failures and dead-lettered tasks for admin UI.
CREATE INDEX idx_enrichment_failed
    ON enrichment_tasks(updated_at DESC)
    WHERE status IN ('failed', 'dead');

-- Stats: daily cap enforcement reads completions in the last 24h.
CREATE INDEX idx_enrichment_completed_at
    ON enrichment_tasks(completed_at)
    WHERE status = 'succeeded';

-- ── Review queue index on items ────────────────────────────────────────────
-- Supports GET /api/admin/enrichment/review ordered by confidence asc (low
-- first), then age asc.
CREATE INDEX idx_items_needs_review
    ON items(classification_confidence NULLS FIRST, updated_at)
    WHERE needs_review = TRUE AND is_deleted = FALSE;
