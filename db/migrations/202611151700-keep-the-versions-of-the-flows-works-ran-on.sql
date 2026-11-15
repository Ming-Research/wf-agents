-- From version 18 of the schema to version 19: each version of a flow a work ran on is kept once, and each work says
-- from which place on it ran on which. A work kept before has none, and is put on the version installed as the engine
-- first starts with them; the cells its records name were all recorded before any table of earlier ids. The version's
-- one row is replaced last.
-- A version of a flow a work ran on, kept once: named by the digest of the files of its definition, with the flow's
-- name and the text of each file by its path from the flow's directory (JSON), as the compiler reads them.
CREATE TABLE flows (
    version TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    files TEXT NOT NULL
);
-- From this place on, the work ran on this version of its flow: as it was added, at place 0, before every other entry,
-- and again each time it went on with another version installed. `ids` says how the cells its records named before
-- read from here on (JSON: each id that names another cell on this version, by the id it is read as). The cell of a
-- run, of a question and of a letting go, and the cell what was written is said to, are kept as they were recorded, and
-- read through each of these after them in turn.
-- A work kept before the versions were has its first as the engine first started with them.
CREATE TABLE on_flow (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    version TEXT NOT NULL REFERENCES flows (version),
    ids TEXT NOT NULL,
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 19;
