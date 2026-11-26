-- From version 34 of the schema to version 35: the engine keeps each call of `run_tests` whose tests the budget of the
-- project's fast workflow kept from their end, refused before it ran or stopped by the run, in a row of the new table
-- `short_of_time`, with each such test in a row of the new table `short_tests`. A call kept before had no budget, and
-- has no row. The version's one row is replaced last.
-- A call of `run_tests` whose tests the budget of the project's fast workflow kept from running to their end: the
-- engine refused it before anything ran, its tests known by the medians of their times in the project's earlier fast
-- runs to take longer than the budget, or the run stopped a test as the budget ran out and did not start those after
-- it. `budget` is the request's budget in whole seconds. Kept once for a call.
CREATE TABLE short_of_time (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    call INTEGER NOT NULL,
    budget INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, call) REFERENCES called (work, call)
);

-- Each test, the `n`th the request named counting from none, that the budget of a call kept in `short_of_time` kept
-- from its end, by the name it was asked for by: `how` is `stopped` (the run stopped it as the budget ran out),
-- `not_started` (the run did not start it, the budget used up) or `refused` (the engine refused the request before
-- it ran); `millis` is how long a stopped test ran, or a refused test's median in the project's earlier fast runs,
-- and null where neither is known.
CREATE TABLE short_tests (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    n INTEGER NOT NULL,
    name TEXT NOT NULL,
    how TEXT NOT NULL CHECK (how IN ('stopped', 'not_started', 'refused')),
    millis INTEGER,
    PRIMARY KEY (work, at, n),
    FOREIGN KEY (work, at) REFERENCES short_of_time (work, at)
);
UPDATE schema_version SET version = 35;
