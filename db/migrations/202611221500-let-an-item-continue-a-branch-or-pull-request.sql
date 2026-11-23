-- From version 22 of the schema to version 23: an item may name a branch or a pull request its work continues, from
-- whose head the work starts. An item kept before continues nothing, and is kept with none. The version's one row is
-- replaced last.
ALTER TABLE items ADD COLUMN continues TEXT;
UPDATE schema_version SET version = 23;
