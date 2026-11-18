-- From version 19 of the schema to version 20: whether the sessions of works that wait are kept warm is the engine's
-- own state, kept here, and no longer a setting read as the engine starts. A database kept before has no row, which
-- says they are kept warm. The version's one row is replaced last.
CREATE TABLE keep_warm (
    n INTEGER PRIMARY KEY,
    warm INTEGER NOT NULL CHECK (warm IN (0, 1)),
    time INTEGER NOT NULL
);
UPDATE schema_version SET version = 20;
