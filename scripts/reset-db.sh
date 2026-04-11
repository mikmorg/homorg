#!/usr/bin/env bash
# Reset the Homorg dev/test database to a clean state.
#
# Removes all user data (items, events, users, sessions, tags, categories, etc.)
# while preserving system seed rows: the root/users containers, the system actor
# user, and the HOM barcode sequence. The backend does NOT need to be restarted —
# schema and migrations are left intact.
#
# Usage:
#   scripts/reset-db.sh              # uses defaults below
#   DB_URL=postgres://... scripts/reset-db.sh

set -euo pipefail

DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-homorg}"
DB_PASS="${DB_PASS:-homorg_dev}"
DB_NAME="${DB_NAME:-homorg}"

PGPASSWORD="$DB_PASS" psql \
	-h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" \
	-v ON_ERROR_STOP=1 \
	--quiet <<'SQL'
BEGIN;
DELETE FROM item_tags;
DELETE FROM scan_sessions;
DELETE FROM camera_tokens;
DELETE FROM invite_tokens;
DELETE FROM refresh_tokens;
TRUNCATE event_store;
DELETE FROM fungible_properties;
DELETE FROM container_properties WHERE item_id NOT IN (
	'00000000-0000-0000-0000-000000000001'::uuid,
	'00000000-0000-0000-0000-000000000002'::uuid
);
DELETE FROM items WHERE id NOT IN (
	'00000000-0000-0000-0000-000000000001'::uuid,
	'00000000-0000-0000-0000-000000000002'::uuid
);
DELETE FROM tags;
DELETE FROM categories;
DELETE FROM container_types;
DELETE FROM users WHERE id <> '00000000-0000-0000-0000-000000000000'::uuid;
DELETE FROM barcode_presets;
UPDATE barcode_sequences SET next_value = 1;

-- Restore container_properties rows for the system containers. These are
-- originally backfilled by migration 0014 from items.is_container=TRUE, so if
-- they're missing the root/users containers get treated as non-containers and
-- user-creation via /auth/setup fails.
INSERT INTO container_properties (item_id)
VALUES
	('00000000-0000-0000-0000-000000000001'::uuid),
	('00000000-0000-0000-0000-000000000002'::uuid)
ON CONFLICT (item_id) DO NOTHING;
COMMIT;
SQL

echo "✓ DB reset complete"
