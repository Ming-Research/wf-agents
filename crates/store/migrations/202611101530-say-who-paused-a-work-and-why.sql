-- From version 1 of the schema to version 2: a pause says who paused the work, the owner or the engine, and why. Every
-- pause kept before was the owner's, with no reason, and is kept so. The table is made anew rather than altered, so
-- that a database brought to version 2 holds the same table as one made with it. The version's one row is replaced last,
-- so that code of any version reads the version it is now, and code of version 1 refuses it.
CREATE TABLE pauses_new (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    paused INTEGER NOT NULL CHECK (paused IN (0, 1)),
    pauser TEXT NOT NULL,
    why TEXT,
    PRIMARY KEY (work, at)
);
INSERT INTO pauses_new (work, at, paused, pauser, why) SELECT work, at, paused, 'Owner', NULL FROM pauses;
DROP TABLE pauses;
ALTER TABLE pauses_new RENAME TO pauses;
UPDATE schema_version SET version = 2;
