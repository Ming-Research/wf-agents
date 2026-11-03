-- Takes a version 77 database to version 78: the health numbers. The engine samples once a minute what no other record
-- holds into the new table samples, and the indexes let the health numbers read a period of the rounds, the changes,
-- the samples, the architect's matters and the blockers by time, the matters a round handled, and the flow's merge of
-- a commit. The engine records each
-- merge it reverted in the new table reverts; the outbox, whose description now says so, is made afresh with its rows,
-- as a migration runs with the foreign keys off. Each statement below that defines something is written exactly as
-- db/schema.sql writes it.

CREATE TABLE samples (
  -- The health numbers that no other record holds, a state at a moment: what the engine samples once a minute, of its
  -- own (the processor time its process used, the reads the owner's page made and how long serving them took, the
  -- works holding a slot and those with a step running) and of each project (how long the oldest matter in its
  -- architect's queue had waited). Every other health number is computed from the records it counts each time it is
  -- read. Written once; a minute the engine did not run has none. The measures, what each is and its unit, are those of
  -- the engine's registry of health numbers.
  -- Read by: the engine's health numbers, which the architect's numbers are made of.
  -- The measure, by its name in the registry, such as engine.cpu.
  measure TEXT NOT NULL,
  -- The project it is of; '' for the engine's own.
  project TEXT NOT NULL DEFAULT '',
  -- The start of the minute it is kept under, and its value: for a state, the minute it was taken in; for an amount
  -- over the time since the sample before, the minute that time began in.
  at INTEGER NOT NULL,
  value REAL NOT NULL,
  -- In this order, so that a measure's samples of a project over a period are read by their time.
  PRIMARY KEY (measure, project, at)
) STRICT;

CREATE TABLE reverts (
  -- Each merge the engine reverted because it broke main: after the check of main after a merge (cell `L1`) found main
  -- broken by it, the engine opened a pull request that reverts it and merged it into main (the outbox's
  -- `revert_merge`). Written once the revert is merged, never changed after.
  -- Read by: the health numbers, which count a commit on main that is a revert apart from one that came onto it
  -- without the flow.
  -- The check of main that found it broken: its work, and its round in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The merge reverted, by the full id of its commit on main.
  merge_commit TEXT NOT NULL,
  -- The revert's pull request, and the full id of the commit its merge put on main.
  pr INTEGER NOT NULL,
  revert_commit TEXT NOT NULL UNIQUE,
  -- When the revert was merged.
  at INTEGER NOT NULL,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE outbox_version_77 AS SELECT * FROM outbox;
DROP TABLE outbox;

CREATE TABLE outbox (
  -- Actions the engine still has to carry out, written in the same transaction as the change that calls for them, so
  -- a crash loses none. A row is deleted once its action is done; what the action did is in rounds, tool_runs or, for
  -- a revert of a merge, reverts. On start the engine first carries out what is left here.
  -- Read by: the engine.
  id INTEGER PRIMARY KEY,
  -- The work the action is for; NULL for one that belongs to no work.
  work INTEGER REFERENCES works(id),
  -- What to do: start a round, run a tool, push, open a pull request, notify the owner, and the like.
  kind TEXT NOT NULL,
  -- Its arguments, JSON.
  args TEXT NOT NULL CHECK (json_valid(args)),
  -- The earliest time to carry it out; a retry waits a while.
  run_after INTEGER NOT NULL,
  -- How many times it has been tried, and the last error.
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  last_error TEXT
) STRICT;

-- The rows keep their ids, by which the engine carries them out in order.
INSERT INTO outbox (id, work, kind, args, run_after, attempts, last_error)
  SELECT id, work, kind, args, run_after, attempts, last_error FROM outbox_version_77;
DROP TABLE outbox_version_77;

-- The next action due.
CREATE INDEX outbox_by_run_after ON outbox (run_after);

-- The rounds that started, and those that ended, within a time: the health numbers read a period by them.
CREATE INDEX rounds_by_start ON rounds (started_at);
CREATE INDEX rounds_by_end ON rounds (ended_at);
-- The merge by the flow, if any, whose commit is a given one: the health numbers tell a commit on main the flow merged
-- from one that came onto it without the flow by it. A statement uses it only when it writes the expression as here.
CREATE INDEX rounds_merge_commit ON rounds (json_extract(output, '$.result.merge_commit')) WHERE transition = 'merged';
-- The samples of a measure within a time, of every project together.
CREATE INDEX samples_by_time ON samples (measure, at);
-- The changes to a table within a time, which the health numbers read a period by.
CREATE INDEX changes_by_time ON changes (table_name, at);
-- The matters of a kind that came within a time, which the health numbers read a period by.
CREATE INDEX architect_matters_by_kind ON architect_matters (kind, at);
-- The matters a round handled, which the health numbers read from the rounds that ended within a time.
CREATE INDEX architect_matters_by_handling ON architect_matters (handled_work, handled_round);
-- The blockers raised within a time, and those that ended after a time, which the health numbers read a period by.
CREATE INDEX blockers_by_time ON blockers (at);
CREATE INDEX blockers_by_end ON blockers (ended_at);
