-- From version 22 of the schema to version 23: the engine keeps when it began to tell the round of a run that was under
-- way to hand in what it has now, because the owner turned to a cell before the run's, so that it tells a round once
-- for a version. A work kept before has none kept. The version's one row is replaced last.
CREATE TABLE hurried (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    block TEXT NOT NULL,
    version INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
UPDATE schema_version SET version = 23;
