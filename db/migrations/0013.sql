-- Takes a version 12 database to version 13: a work's pause says who paused it, the owner or the engine, so that the
-- owner's Start all lets go on only the works the owner paused. Each statement below that defines something is
-- written exactly as db/schema.sql writes it.

ALTER TABLE works ADD COLUMN paused_by TEXT CHECK (paused_by IN ('owner', 'engine'));

-- A work paused before says who paused it by the change that last set its pause; a pause no change records is the
-- engine's. The change is the migration's, not a writer's, so the trigger that records changes to works is dropped
-- first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_works_update;
UPDATE works SET paused_by = CASE (
  SELECT c.actor FROM changes c
  WHERE c.table_name = 'works' AND json_extract(c.row_key, '$.id') = works.id
    AND json_extract(c.new_row, '$.paused') IS NOT NULL
  ORDER BY c.id DESC LIMIT 1) WHEN 'owner' THEN 'owner' ELSE 'engine' END
WHERE paused IS NOT NULL;
