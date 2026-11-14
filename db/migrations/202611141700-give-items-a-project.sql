-- From version 11 of the schema to version 12: an item belongs to the project it was added to, by the name the
-- installation's settings give it. An item kept before belonged to the one project the installation had, which had no
-- name, and is kept with none. The version's one row is replaced last, so that code of any version reads the version
-- it is now, and code of version 11 refuses it.
ALTER TABLE items ADD COLUMN project TEXT;
UPDATE schema_version SET version = 12;
