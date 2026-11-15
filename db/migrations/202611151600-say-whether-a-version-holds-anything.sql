-- From version 17 of the schema to version 18: a write of a block says whether the version holds nothing, with no
-- value and its content empty or only whitespace, which a test of content reads. A write kept before has the column
-- as 0 here; the store then sets it from the content the write has, once this has run (`mark_empty`). The version's
-- one row is replaced last.
ALTER TABLE writes ADD COLUMN empty INTEGER NOT NULL DEFAULT 0 CHECK (empty IN (0, 1));
ALTER TABLE written ADD COLUMN empty INTEGER NOT NULL DEFAULT 0 CHECK (empty IN (0, 1));
UPDATE schema_version SET version = 18;
