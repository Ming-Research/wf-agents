-- From version 5 of the schema to version 6: each round says the context it ended with and how often its session was
-- compacted; each new session of a line that began is recorded, and what each try of a round ran on. A round kept
-- before says neither: its context is not known, it is counted as compacted never, and what it ran on is not recorded.
-- The columns are added at the end of the table, as a new file has them. The version's one row is replaced last, so
-- that code of any version reads the version it is now.
ALTER TABLE spent ADD COLUMN context_tokens INTEGER;
ALTER TABLE spent ADD COLUMN compactions INTEGER NOT NULL DEFAULT 0;
CREATE TABLE sessions (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    line TEXT NOT NULL,
    why TEXT NOT NULL,
    PRIMARY KEY (work, at)
);
CREATE TABLE ran (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    attempt INTEGER NOT NULL,
    level TEXT NOT NULL,
    model TEXT NOT NULL,
    effort TEXT,
    account TEXT,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
UPDATE schema_version SET version = 6;
