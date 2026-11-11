-- From version 3 of the schema to version 4: a work that has ended says when what it held outside the engine was let
-- go. No work kept before had let go of anything, so the table is made empty. The version's one row is replaced last,
-- so that code of any version reads the version it is now, and code of version 3 refuses it.
CREATE TABLE released (
    work INTEGER PRIMARY KEY REFERENCES works (number),
    at INTEGER NOT NULL
);
UPDATE schema_version SET version = 4;
