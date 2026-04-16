-- Migration 0027: Add photo_needed flag for camera auto-trigger
--
-- Distinguishes item creation (which should auto-open camera) from
-- item moves and container creation (which should not).

ALTER TABLE scan_sessions ADD COLUMN photo_needed BOOLEAN NOT NULL DEFAULT FALSE;
