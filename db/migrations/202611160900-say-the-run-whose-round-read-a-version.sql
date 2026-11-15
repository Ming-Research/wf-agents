-- From version 18 of the schema to version 19: a write of a block says the run whose round's carrier read what was
-- written, where a round's did, so that what a round knew of a block is told apart from a version it never knew.
-- Writes kept before say none, as no round's.
ALTER TABLE written ADD COLUMN read_by INTEGER;
UPDATE schema_version SET version = 19;
