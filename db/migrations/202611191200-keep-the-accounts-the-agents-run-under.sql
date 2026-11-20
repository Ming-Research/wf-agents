-- From version 20 of the schema to version 21: the accounts the agents' program may run under, each a name the owner
-- gives with a token kept outside the store, are kept as they are added, removed and taken, the engine's own state and
-- no setting; a database kept before has none, and its agents run on the sign-in of their directory. A try of a round
-- records the directory of the program's settings it ran with and the account it ran under apart: the table of tries
-- is made anew with both, every row kept before carried across whole with what it held as its account, which was the
-- directory, as its directory, and no account. The version's one row is replaced last.
CREATE TABLE accounts (
    n INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    what TEXT NOT NULL CHECK (what IN ('Added', 'Removed', 'Taken')),
    time INTEGER NOT NULL
);
CREATE TABLE ran_new (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    attempt INTEGER NOT NULL,
    level TEXT NOT NULL,
    model TEXT NOT NULL,
    effort TEXT,
    directory TEXT,
    account TEXT,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
INSERT INTO ran_new (work, at, run, round, attempt, level, model, effort, directory, account)
    SELECT work, at, run, round, attempt, level, model, effort, account, NULL FROM ran;
DROP TABLE ran;
ALTER TABLE ran_new RENAME TO ran;
UPDATE schema_version SET version = 21;
