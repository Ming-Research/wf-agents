-- From version 31 of the schema to version 32: the engine keeps, beside what a work's time went to as it ended, each
-- stretch the work held the turn of its project to check and merge, so that the time of the project's other works
-- that waited for the turn is still drawn as a wait for it once the flow the work ended on is no longer installed and
-- the turn can no longer be worked out. A work kept before has none kept (NULL), which is read as one that never held
-- it. The version's one row is replaced last.
ALTER TABLE kept_time ADD COLUMN turn TEXT;
UPDATE schema_version SET version = 32;
