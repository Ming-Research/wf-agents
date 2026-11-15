-- From version 17 of the schema to version 18: a write of a block says whether the version holds nothing, with no
-- value and its content empty or only whitespace, which a test of content reads. The tables of writes are made anew
-- with the column, the rows kept before are put aside, and the store fills the new tables from them as it runs this,
-- each row with the column set from the content its version has, and then takes them out (`fill_empty`). The
-- version's one row is replaced last.
ALTER TABLE writes RENAME TO writes_before_18;
ALTER TABLE written RENAME TO written_before_18;
CREATE TABLE writes (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    n INTEGER NOT NULL,
    block TEXT NOT NULL,
    matters INTEGER NOT NULL CHECK (matters IN (0, 1)),
    value TEXT,
    empty INTEGER NOT NULL DEFAULT 0 CHECK (empty IN (0, 1)),
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
    empty INTEGER NOT NULL DEFAULT 0 CHECK (empty IN (0, 1)),
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 18;
