-- From version 3 of the schema to version 4: the owner's pause of every work is kept, so that it holds across a stop of
-- the engine. A database made before has none, and the works it holds go on as they did: each held back by its own
-- pause alone. The version's one row is replaced last, so that code of any version reads the version it is now, and
-- code of version 3 refuses it.
CREATE TABLE everything_paused (
    n INTEGER PRIMARY KEY,
    paused INTEGER NOT NULL CHECK (paused IN (0, 1)),
    time INTEGER NOT NULL
);
UPDATE schema_version SET version = 4;
