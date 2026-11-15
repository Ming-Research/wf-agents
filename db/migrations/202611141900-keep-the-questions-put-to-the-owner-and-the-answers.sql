-- From version 12 of the schema to version 13: the questions put to the owner, each version of each, and the owner's
-- answers are kept as entries of their own. No work kept before had a question of its own, so both tables are made
-- empty. The version's one row is replaced last.
CREATE TABLE questions (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    name TEXT NOT NULL,
    cell TEXT NOT NULL,
    run INTEGER NOT NULL,
    engine INTEGER NOT NULL CHECK (engine IN (0, 1)),
    waits INTEGER NOT NULL CHECK (waits IN (0, 1)),
    withdrawn INTEGER NOT NULL CHECK (withdrawn IN (0, 1)),
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
CREATE TABLE answers (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    question TEXT NOT NULL,
    version INTEGER NOT NULL,
    option INTEGER NOT NULL,
    why TEXT,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, version) REFERENCES questions (work, at)
);
UPDATE schema_version SET version = 13;
