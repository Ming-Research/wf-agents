-- From version 13 of the schema to version 14: each change of an item says why it was made; an item carries tags, as it
-- is added and by its changes; and a cancelled item may be reopened, so that a work may be cancelled, have what it held
-- let go and be reopened more than once. A change kept before gave no reason, and is kept with none; an item kept
-- before carries no tag. The tables of cancels and of what was let go, which held one row for a work, are made again
-- keyed by the work and the place, their rows kept as they were. The version's one row is replaced last, so that code
-- of any version reads the version it is now.
ALTER TABLE item_changes ADD COLUMN why TEXT;
CREATE TABLE tagged (
    item INTEGER NOT NULL REFERENCES items (number),
    tag TEXT NOT NULL,
    PRIMARY KEY (item, tag)
);
CREATE TABLE cancelled_again (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    why TEXT,
    PRIMARY KEY (work, at)
);
INSERT INTO cancelled_again (work, at, why) SELECT work, at, why FROM cancelled;
DROP TABLE cancelled;
ALTER TABLE cancelled_again RENAME TO cancelled;
CREATE TABLE reopened (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    why TEXT NOT NULL,
    PRIMARY KEY (work, at)
);
CREATE TABLE released_again (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    PRIMARY KEY (work, at)
);
INSERT INTO released_again (work, at) SELECT work, at FROM released;
DROP TABLE released;
ALTER TABLE released_again RENAME TO released;
UPDATE schema_version SET version = 14;
