-- From version 20 of the schema to version 21: the accounts the agents' program may run under, each a name the owner
-- gives with a token kept outside the store, are kept here as they are added, removed and taken, the engine's own
-- state and no setting. A database kept before has no row, which says no account is added and the agents run on the
-- sign-in of their directory. The version's one row is replaced last.
CREATE TABLE accounts (
    n INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    what TEXT NOT NULL CHECK (what IN ('Added', 'Removed', 'Taken')),
    time INTEGER NOT NULL
);
UPDATE schema_version SET version = 21;
