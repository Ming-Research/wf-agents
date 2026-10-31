-- Takes a version 47 database to version 48: a work records the kind of its end beside the text of why (ended_kind),
-- written by the one function that ends a work, so that the engine counts the works that ended without a merge by
-- kind. Each statement below that defines something is written exactly as db/schema.sql writes it.

-- The change below is the migration's, not a writer's: the trigger that records changes to a work is dropped first, and
-- every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_works_update;

ALTER TABLE works ADD COLUMN ended_kind TEXT CHECK (ended_kind IS NULL OR ended_kind IN ('item_cancelled', 'flow_moved', 'started_over', 'other'));

-- The kind of the works already stopped is read once from the start of their text, as every writer before this one
-- wrote it: a cancelled item's, the flow moving to a new version, a work started over; any other text is other.
UPDATE works SET ended_kind = CASE
  WHEN substr(ended, 1, 14) = 'item cancelled' THEN 'item_cancelled'
  WHEN instr(ended, 'the flow moved') > 0 THEN 'flow_moved'
  WHEN instr(ended, 'started it over') > 0 THEN 'started_over'
  ELSE 'other' END
WHERE ended IS NOT NULL;
