-- From version 10 of the schema to version 11: each text of `observations` in the return a run was taken on is kept as
-- an observation of the installation, numbered as it is kept, and the owner's reading of one is kept. The observations
-- of the runs that finished before are kept now, numbered in the order the runs finished, and are unread. The version's
-- one row is replaced last.
CREATE TABLE observations (
    number INTEGER PRIMARY KEY,
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    n INTEGER NOT NULL,
    UNIQUE (work, run, n),
    FOREIGN KEY (work, run, round) REFERENCES back (work, run, round)
);
CREATE TABLE observations_read (
    observation INTEGER PRIMARY KEY REFERENCES observations (number),
    at INTEGER NOT NULL
);
-- The return a run was taken on is what came back at the end of its last round. A run finishes in the moment its
-- return is taken, so the time of the moment its finish is in orders them.
INSERT INTO observations (work, run, round, n)
SELECT b.work, b.run, b.round, o.key + 1
FROM finished f
JOIN back b ON b.work = f.work AND b.run = f.run
JOIN json_each(b.back, '$.Returned.said.observations') o
WHERE b.round = (SELECT max(l.round) FROM back l WHERE l.work = f.work AND l.run = f.run)
ORDER BY
    (SELECT m.time FROM moments m WHERE m.work = f.work AND m.place <= f.at ORDER BY m.place DESC LIMIT 1),
    f.work,
    f.at,
    o.key;
UPDATE schema_version SET version = 11;
