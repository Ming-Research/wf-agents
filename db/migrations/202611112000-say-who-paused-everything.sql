-- From version 6 of the schema to version 7: the pause of everything says who set it, the owner or the engine as it
-- started with works going on. Every row kept before was the owner's, and is kept so. The table is made anew rather
-- than altered, so that a database brought to version 7 holds the same table as one made with it. The version's one
-- row is replaced last, so that code of any version reads the version it is now, and code of version 6 refuses it.
CREATE TABLE everything_paused_new (
    n INTEGER PRIMARY KEY,
    paused INTEGER NOT NULL CHECK (paused IN (0, 1)),
    pauser TEXT NOT NULL,
    time INTEGER NOT NULL
);
INSERT INTO everything_paused_new (n, paused, pauser, time) SELECT n, paused, 'Owner', time FROM everything_paused;
DROP TABLE everything_paused;
ALTER TABLE everything_paused_new RENAME TO everything_paused;
UPDATE schema_version SET version = 7;
