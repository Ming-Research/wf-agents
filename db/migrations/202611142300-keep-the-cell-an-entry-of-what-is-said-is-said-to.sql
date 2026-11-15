-- From version 15 of the schema to version 16: a write is kept with the cell it is said to, where it is said to one, as
-- the owner's entry of the record of what is said is. A write kept before is said to none. The version's one row is
-- replaced last.
ALTER TABLE written ADD COLUMN said_to TEXT;
UPDATE schema_version SET version = 16;
