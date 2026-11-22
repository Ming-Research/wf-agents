-- From version 22 of the schema to version 23: the processes the program of a try left running in its group as it
-- ended, which its carrier stopped, are counted. A try kept before left none that was counted.
CREATE TABLE left_behind (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    attempt INTEGER NOT NULL,
    processes INTEGER NOT NULL CHECK (processes > 0),
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
UPDATE schema_version SET version = 23;
