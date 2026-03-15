-- Migration 0018: Seed system actor
--
-- The barcode-generation commands emit BarcodeGenerated events with
-- actor_id = '00000000-0000-0000-0000-000000000000' (Uuid::nil) to
-- represent background/system operations with no human actor.
--
-- The event_store.actor_id column is a nullable FK → users(id), so NULL is
-- allowed for truly anonymous events; however Rust passes Uuid::nil() rather
-- than Option::None at every call-site.  Inserting a sentinel system user row
-- satisfies the FK without requiring code-level changes.
--
-- The row is intentionally inert:
--   • password_hash is an empty string — no valid Argon2 hash will ever match it.
--   • is_active = FALSE prevents the row from appearing in any "active users" query.
--   • role = 'readonly' is the least-privileged value allowed by the CHECK constraint.

INSERT INTO users (id, username, password_hash, display_name, role, is_active)
VALUES (
    '00000000-0000-0000-0000-000000000000',
    'system',
    '',                     -- Un-authenticatable: no valid Argon2 hash is empty.
    'System',
    'readonly',
    FALSE
)
ON CONFLICT (id) DO NOTHING;
