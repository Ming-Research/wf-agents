-- From version 16 of the schema to version 17: what the runs of the project's tests that a call of `run_tests` asked
-- for reported is kept, where its host read each test's result. A call kept before has none, and a result a return
-- says a run gave is held to no run of it. The version's one row is replaced last.
CREATE TABLE tested (
    work INTEGER NOT NULL,
    call INTEGER NOT NULL,
    tested TEXT NOT NULL, -- Hidden from agents.
    PRIMARY KEY (work, call),
    FOREIGN KEY (work, call) REFERENCES called (work, call)
);
UPDATE schema_version SET version = 17;
