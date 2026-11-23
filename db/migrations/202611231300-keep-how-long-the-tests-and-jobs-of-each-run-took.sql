-- From version 28 of the schema to version 29: the engine keeps how long each test and each job of a run of the
-- project's CI took, as the host read it, so that tests and checks are judged by what earlier ones took. A work kept
-- before has none kept. The version's one row is replaced last.
CREATE TABLE test_names (
    test INTEGER PRIMARY KEY,
    unit TEXT NOT NULL,
    name TEXT NOT NULL,
    UNIQUE (unit, name)
);
CREATE TABLE timed (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    asking TEXT NOT NULL,
    source TEXT NOT NULL,
    commit_id TEXT NOT NULL,
    workflow TEXT NOT NULL,
    PRIMARY KEY (work, at)
);
CREATE TABLE timed_jobs (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    name TEXT NOT NULL,
    millis INTEGER NOT NULL,
    PRIMARY KEY (work, at, name),
    FOREIGN KEY (work, at) REFERENCES timed (work, at)
);
CREATE TABLE timed_tests (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    test INTEGER NOT NULL REFERENCES test_names (test),
    passed INTEGER NOT NULL,
    millis INTEGER NOT NULL,
    PRIMARY KEY (work, at, test),
    FOREIGN KEY (work, at) REFERENCES timed (work, at)
);
UPDATE schema_version SET version = 29;
