-- From version 9 of the schema to version 10: a finished run says what of the blocks that ran it its return dealt with;
-- a version put into a round under way is kept; and so is what was filled into the session of each round as it began.
-- A run kept before says nothing of what it dealt with, and reads as having dealt with everything that ran it, as each
-- did; a round kept before has nothing delivered and no record of what was filled. The column is added at the end of
-- the table's columns, as a new file has it. The version's one row is replaced last.
ALTER TABLE finished ADD COLUMN dealt_with TEXT;
CREATE TABLE delivered (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    block TEXT NOT NULL,
    version INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
CREATE TABLE filled (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    filled TEXT NOT NULL,
    PRIMARY KEY (work, run, round),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
UPDATE schema_version SET version = 10;
