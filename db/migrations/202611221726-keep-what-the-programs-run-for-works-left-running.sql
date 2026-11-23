-- From version 23 of the schema to version 24: the processes a program the engine ran for a work left running in its
-- group as it ended, which its carrier stopped, are counted. A program that ended before left none that was counted.
CREATE TABLE left_behind (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    of TEXT NOT NULL,
    processes INTEGER NOT NULL CHECK (processes > 0),
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 24;
