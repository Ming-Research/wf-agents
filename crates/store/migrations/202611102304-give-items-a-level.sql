-- From version 2 of the schema to version 3: an item has the level it was given as it was added, and the owner may put
-- it at another, each change kept. An item kept before was given none, and is kept with none. The version's one row is
-- replaced last, so that code of any version reads the version it is now, and code of version 2 refuses it.
ALTER TABLE items ADD COLUMN level TEXT;
CREATE TABLE level_changes (
    item INTEGER NOT NULL REFERENCES items (number),
    n INTEGER NOT NULL,
    level TEXT NOT NULL,
    at INTEGER NOT NULL,
    PRIMARY KEY (item, n)
);
UPDATE schema_version SET version = 3;
