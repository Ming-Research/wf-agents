-- From version 23 of the schema to version 24: each time an account was taken as the one the agents run under says who
-- took it, the owner or the engine, and, where the engine took it, when the usage limit of the account it left resets,
-- where that was said; every row kept before was the owner's, and says no reset. Whether the engine may move to another
-- account at a usage limit is its own state, kept here, off until the owner switches it on. The version's one row is
-- replaced last.
ALTER TABLE accounts ADD COLUMN taker TEXT NOT NULL DEFAULT 'Owner' CHECK (taker IN ('Owner', 'Engine'));
ALTER TABLE accounts ADD COLUMN left_resets INTEGER;
CREATE TABLE moving_on (
    n INTEGER PRIMARY KEY,
    moving INTEGER NOT NULL CHECK (moving IN (0, 1)),
    time INTEGER NOT NULL
);
UPDATE schema_version SET version = 25;
