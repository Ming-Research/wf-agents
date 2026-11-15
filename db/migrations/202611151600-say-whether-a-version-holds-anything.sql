-- From version 17 of the schema to version 18: a write of a block says whether the version holds nothing, with no
-- value and its content empty or only whitespace, which a test of content reads. The tables of writes are made anew
-- with the column, the rows kept before are put aside, and the store step of this migration (`tables::fill_empty`)
-- copies each into the new tables whole with the column set from the content its version has, and takes the old
-- tables out. The version's one row is replaced last, here.
ALTER TABLE writes RENAME TO writes_before_18;
ALTER TABLE written RENAME TO written_before_18;
CREATE TABLE writes (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    n INTEGER NOT NULL,
    block TEXT NOT NULL,
    matters INTEGER NOT NULL CHECK (matters IN (0, 1)),
    value TEXT,
    empty INTEGER NOT NULL CHECK (empty IN (0, 1)),
    PRIMARY KEY (work, run, n),
    FOREIGN KEY (work, run) REFERENCES finished (work, run)
);
CREATE TABLE written (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    block TEXT NOT NULL,
    writer TEXT NOT NULL,
    matters INTEGER NOT NULL CHECK (matters IN (0, 1)),
    value TEXT,
    said_to TEXT,
    empty INTEGER NOT NULL CHECK (empty IN (0, 1)),
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 18;
