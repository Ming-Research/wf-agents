-- From version 16 of the schema to version 17: a finished run says the block written from outside the work, a new
-- version of which what it wrote waits for, where the engine's own act says one. A run kept before says none, and
-- waits for nothing. The version's one row is replaced last.
ALTER TABLE finished ADD COLUMN waits_for TEXT;
UPDATE schema_version SET version = 17;
