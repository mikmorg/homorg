-- Migration 0012: Make system_barcode optional on items
--
-- Purpose: barcodes should only be attached when a user provides one (pre-printed labels,
-- physical scanner workflow) or explicitly requests a generated one.  Items can now exist
-- without a system barcode and have one assigned later via the POST /items/{id}/barcode
-- endpoint.
--
-- Changes:
--  1. DROP NOT NULL constraint on items.system_barcode
--  2. PostgreSQL UNIQUE constraints allow multiple NULL values, so no change to that index.
--  3. Update seed rows: both ROOT/USERS containers keep their barcodes unchanged.

-- Allow items to be created without a system barcode
ALTER TABLE items ALTER COLUMN system_barcode DROP NOT NULL;

-- Ensure the unique index explicitly handles the text type (existing index is fine, no change needed)
-- NULL values are not considered equal by the UNIQUE constraint in PostgreSQL, so having multiple
-- barcode-less items is permitted.

