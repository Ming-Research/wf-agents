-- From version 20 of the schema to version 21: each version of a flow a work runs on is kept once, as its files, each
-- file once, and its outline; each work says from which place on it ran on which version; and what a work's time went
-- to is kept as it ends. A work kept before has none of them, and is put on a version, or on none, as the engine first
-- starts with them. The version's one row is replaced last.
CREATE TABLE flow_files (
    digest TEXT PRIMARY KEY,
    text TEXT NOT NULL
);
CREATE TABLE flows (
    version TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    files TEXT NOT NULL,
    outline TEXT NOT NULL
);
CREATE TABLE on_flow (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    version TEXT REFERENCES flows (version),
    PRIMARY KEY (work, at)
);
CREATE TABLE kept_time (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    stretches TEXT NOT NULL,
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 21;
