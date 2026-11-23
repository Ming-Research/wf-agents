-- From version 26 of the schema to version 27: the engine is stopped by being asked to, once what is under way has
-- ended or at once, and that is kept, with its start after it. A database kept before has no row, which says no stop
-- stands. The version's one row is replaced last.
CREATE TABLE stops (
    n INTEGER PRIMARY KEY,
    what TEXT NOT NULL CHECK (what IN ('Asked', 'Now', 'Over')),
    time INTEGER NOT NULL
);
UPDATE schema_version SET version = 27;
