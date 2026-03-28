-- H-16: Fix duplicate FK constraints on users.container_id
-- 0003 created fk_users_container_id (NOT deferrable)
-- 0011 tried to replace users_container_id_fkey (wrong name) and added a second constraint
-- This migration drops the stale non-deferrable constraint, leaving only the
-- correct deferrable one from 0011.
ALTER TABLE users
    DROP CONSTRAINT IF EXISTS fk_users_container_id;
