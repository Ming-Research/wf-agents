-- From version 18 of the schema to version 19: the pull request the host of a work's repository found the work checked
-- through is kept with the work. A work kept before has none kept. The version's one row is replaced last.
CREATE TABLE pulls (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    number INTEGER NOT NULL,
    url TEXT NOT NULL,
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 19;
