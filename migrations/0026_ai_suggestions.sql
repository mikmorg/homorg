-- 0026: Pending AI suggestions column + re-seed ai-enricher system user.
--
-- ai_suggestions stores a JSON blob shaped like AiSuggestions (see
-- src/models/enrichment.rs) representing the most recent proposal from the
-- enricher daemon, pending admin review. NULL means "nothing pending".
--
-- Populated by the daemon when it runs enrichment against a user-edited item
-- or produces a low-confidence result; cleared by the admin approve/reject
-- actions (which also flip needs_review back to FALSE).

ALTER TABLE items
    ADD COLUMN IF NOT EXISTS ai_suggestions JSONB;

COMMENT ON COLUMN items.ai_suggestions IS
    'Pending AI-proposed changes awaiting admin review. NULL when nothing pending. Shape: AiSuggestions in src/models/enrichment.rs.';

-- Review-queue support: "items with pending suggestions, worst-confidence first".
-- The existing idx_items_needs_review (from 0025) covers the needs_review+confidence axis;
-- this partial index narrows it to the rows that actually have a suggestion blob.
CREATE INDEX IF NOT EXISTS idx_items_ai_suggestions_pending
    ON items (updated_at DESC)
    WHERE ai_suggestions IS NOT NULL
      AND needs_review = TRUE
      AND is_deleted = FALSE;

-- Belt-and-braces: re-insert the ai-enricher system user in case dev DB resets
-- (or scripts/reset-db.sh re-applying migrations mid-flight) wiped the row that
-- 0025 originally inserted. ON CONFLICT guards against the happy path.
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
