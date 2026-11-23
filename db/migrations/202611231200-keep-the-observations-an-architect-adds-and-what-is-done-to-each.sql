-- From version 24 of the schema to version 25: an observation may be one an architect added, with a text, the project
-- it is of and the project that reported it, as well as one a run kept as it finished; each is given a title and tags
-- as it is added, and has its later changes kept with why, in the order they were made. The table of the observations
-- is made anew so that the columns of a run's observation may be null, and carries each row across whole with its
-- number; the owner's readings are carried across with them, the table of readings being taken away while the
-- observations are made anew, since it names them. The version's one row is replaced last.
CREATE TEMP TABLE kept_readings AS SELECT observation, at FROM observations_read;
DROP TABLE observations_read;
CREATE TABLE observations_next (
    number INTEGER PRIMARY KEY,
    work INTEGER,
    run INTEGER,
    round INTEGER,
    n INTEGER,
    text TEXT,
    project TEXT,
    reported_by TEXT,
    added_at INTEGER,
    title TEXT,
    UNIQUE (work, run, round, n),
    FOREIGN KEY (work, run, round) REFERENCES back (work, run, round),
    CHECK (
        (work IS NOT NULL AND run IS NOT NULL AND round IS NOT NULL AND n IS NOT NULL
            AND text IS NULL AND project IS NULL AND reported_by IS NULL AND added_at IS NULL AND title IS NULL)
        OR (work IS NULL AND run IS NULL AND round IS NULL AND n IS NULL
            AND text IS NOT NULL AND project IS NOT NULL AND reported_by IS NOT NULL AND added_at IS NOT NULL)
    )
);
INSERT INTO observations_next (number, work, run, round, n) SELECT number, work, run, round, n FROM observations;
DROP TABLE observations;
ALTER TABLE observations_next RENAME TO observations;
CREATE TABLE observations_read (
    observation INTEGER PRIMARY KEY REFERENCES observations (number),
    at INTEGER NOT NULL
);
INSERT INTO observations_read (observation, at) SELECT observation, at FROM kept_readings;
DROP TABLE kept_readings;
CREATE TABLE observation_tagged (
    observation INTEGER NOT NULL REFERENCES observations (number),
    tag TEXT NOT NULL,
    PRIMARY KEY (observation, tag)
);
CREATE TABLE observation_changes (
    observation INTEGER NOT NULL REFERENCES observations (number),
    n INTEGER NOT NULL,
    at INTEGER NOT NULL,
    change TEXT NOT NULL,
    why TEXT NOT NULL,
    PRIMARY KEY (observation, n)
);
UPDATE schema_version SET version = 25;
