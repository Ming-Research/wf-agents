-- From version 37 of the schema to version 38: the slots, which bound how many rounds of agents go on at once, are kept
-- each time they are set, so that how many were taken can be read against how many there were at the time; a job of a
-- run says how long it waited for a runner before it started, `queued_millis`; and what a work's time went to, kept as
-- it ended, says whether it tells a wait for a slot apart from the other waits, `separated`. A database kept before has
-- no slots set, which says nothing was recorded of them; its jobs have no time of waiting (NULL), which says it was not
-- recorded, and the time kept of each work that ended does not tell a wait for a slot apart (0). The version's one row
-- is changed last.
CREATE TABLE slots_set (
    n INTEGER PRIMARY KEY,
    slots INTEGER NOT NULL,
    time INTEGER NOT NULL
);
ALTER TABLE timed_jobs ADD COLUMN queued_millis INTEGER;
ALTER TABLE kept_time ADD COLUMN separated INTEGER NOT NULL DEFAULT 0;
UPDATE schema_version SET version = 38;
