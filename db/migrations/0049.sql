-- Takes a version 48 database to version 49: a work on an older flow version that goes on moves to the version the
-- engine runs, at its cell and in its sessions, when that cell and the shape of every return it handed in are unchanged
-- there, instead of always being taken over by a new work; the works say which version a work last moved from, and the
-- flow versions say so. The engine reminds the owner once an interval of the asks waiting for the owner's answer, and
-- the engine's notices say which are such reminders, from the last of which the next is timed. Those tables are made
-- afresh with their rows. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE flow_versions_version_48 AS SELECT * FROM flow_versions;
DROP TABLE flow_versions;

CREATE TABLE flow_versions (
  -- Every compiled flow a work has run on. A work runs on the version it started with until the flow moves to a new
  -- version; then, when a work on an older one next goes on, it moves to the new one at its cell when that cell and
  -- the shape of every return it handed in are unchanged there, and otherwise it stops and its item starts again on
  -- the new one. Old rounds stay readable after the flow changes.
  -- Read by: the engine; people and agents reading old rounds.
  id INTEGER PRIMARY KEY,
  -- Hash of compiled, so each version is stored once.
  digest TEXT NOT NULL UNIQUE,
  -- The compiler's expanded flow, JSON: every graph's cells, inputs, return schemas and transition rows.
  compiled TEXT NOT NULL CHECK (json_valid(compiled))
) STRICT;

INSERT INTO flow_versions (id, digest, compiled)
SELECT id, digest, compiled
FROM flow_versions_version_48;
DROP TABLE flow_versions_version_48;

CREATE TABLE works_version_48 AS SELECT * FROM works;
DROP TABLE works;

CREATE TABLE works (
  -- One run of a graph of the flow: an engineer's work on an item, or one work of a project's architect; a flow version
  -- before the setting of a project's policy was a stage of the architect's works ran it as a work of its own, of the
  -- graph policy, which the engine takes over when it next goes on. It holds the current
  -- state the engine needs to take the next step, written in the same transaction as the round that changed it; the
  -- history is in rounds. Which cell brought the work to its cell, whether it is done, what it waits for and when it
  -- started and ended are derived from its rounds. At most one work per item runs at a time.
  -- Read by: the engine at every step.
  id INTEGER PRIMARY KEY,
  -- The item worked on; NULL for a work of an architect, which belongs to no item but to its project.
  item INTEGER REFERENCES items(id),
  -- The flow version it runs on: the one it started with, or the one it last moved to.
  flow_version INTEGER NOT NULL REFERENCES flow_versions(id),
  -- The flow version it last moved from to the one it runs on, when it went on after the flow moved to a new version
  -- and its cell and the shape of every return it had handed in were unchanged there, so that it went on at its cell
  -- in its sessions instead of a new work taking its item over; NULL for a work that never moved.
  -- Read by: the owner's interface, which says the work moved to its version.
  moved_from INTEGER REFERENCES flow_versions(id),
  -- Which graph of that version it runs. Read by: the engine.
  graph TEXT NOT NULL CHECK (graph IN ('engineer', 'architect', 'policy')),
  -- The cell the work is at. Read by: the engine; views of status and slots.
  at TEXT NOT NULL,
  -- The cell the work left to ask or to be answered; the answer resumes it there. NULL when nothing is pending.
  -- Read by: the engine; cell `A4`, to know where the question was asked.
  resume_at TEXT,
  -- The cells waiting for an answer, innermost last, JSON list. Read by: the engine, to route answers back.
  waiting TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(waiting)),
  -- Values earlier transition rows recorded, JSON object: whether full CI has passed, whether this design has been
  -- pushed, whether a dedicated adversarial test is needed, and the like.
  -- Read by: the engine; cells `D2`, `I1` and `T7` through their inputs.
  recorded TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(recorded)),
  -- The machine every round of the work runs on; set when the work starts. Read by: the engine.
  machine TEXT,
  -- The worktree path on that machine and the branch, created when the work starts and removed when it ends.
  -- Read by: the engine and the session manager on that machine.
  worktree TEXT,
  branch TEXT,
  -- Pull request number; NULL before the first push. Read by: cells `M2` and `X1`; people.
  pr INTEGER,
  -- The head last pushed; the next push is checked against it. Read by: cells `T2`, `M2`, `X1`
  -- and `T7`.
  pushed_head TEXT,
  -- Why the work is paused: the engine's reason, when a step could not be taken or returns were sent back too often,
  -- or the owner's words; NULL while it goes on. Read by: the engine; the slots view.
  paused TEXT,
  -- Why the work was stopped before its end cell, for example because its item was cancelled; NULL otherwise.
  -- Read by: the engine; the status view.
  ended TEXT,
  -- Who paused the work, the owner or the engine; NULL while it goes on. The owner's Start all lets go on only the
  -- works the owner paused: a work the engine paused waits for the owner to look at it and let it go on by itself.
  -- Read by: the engine, when the owner lets works go on; the owner's interface.
  paused_by TEXT CHECK (paused_by IN ('owner', 'engine')),
  -- For a work of the architect, or of an earlier version's setting of a policy, the project whose architect it is;
  -- NULL for an engineer's work, whose project is its item's. Read by: the engine, which runs one work of each
  -- project's architect at a time, and reads the project's views for it; the architect's views; the owner's
  -- interface.
  project TEXT CHECK (project IS NULL OR graph IN ('architect', 'policy'))
) STRICT;

INSERT INTO works (id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head, paused, ended, paused_by, project)
SELECT id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head, paused, ended, paused_by, project
FROM works_version_48;
DROP TABLE works_version_48;
CREATE INDEX works_by_item ON works (item);

CREATE TABLE engine_notices_version_48 AS SELECT * FROM engine_notices;
DROP TABLE engine_notices;

CREATE TABLE engine_notices (
  -- What the engine tells the owner of its own accord, one row for each notice: a work stopped by a new flow version,
  -- a design card posted, every work halted until a usage limit resets, a fault of wf-agents, the asks still waiting
  -- for the owner's answer, and the like. It needs no answer. The same text goes to the notifications log.
  -- Hidden from agents.
  -- Read by: the owner's interface, which shows those unread with the messages agents wrote; the engine, for a fault
  -- of wf-agents (fault) and for when it last reminded the owner of the asks waiting (reminder).
  id INTEGER PRIMARY KEY,
  -- The work it is about; NULL for one about no work.
  work INTEGER REFERENCES works(id),
  -- The notice, as the engine wrote it.
  text TEXT NOT NULL,
  -- When the engine wrote it.
  at INTEGER NOT NULL,
  -- When the owner marked it read, or forwarded the fault it reports to an architect, by hand or by the owner's
  -- setting; NULL while it is unread. Read by: the owner's interface.
  read_at INTEGER,
  -- When the notice is of a new ask of the owner: that ask, for the owner's interface to link to. Read by: the
  -- owner's interface; the engine, which hands a flow anomaly's ask to the architect it is forwarded to.
  ask INTEGER REFERENCES owner_asks(id),
  -- When the notice reports a fault of wf-agents, its kind: cache_miss, rounds that missed their session's prompt
  -- cache, each kept in cache_misses; flow_anomaly, an ask of the owner that came of a row counting a flow anomaly,
  -- where the way on an engineer chose did not fit the situation. wf-agents runs other projects too, and such a fault
  -- is its own, to be fixed in wf-agents whichever project it came up in. NULL for another notice. Read by: the
  -- owner's interface, which sets such a notice apart and forwards it to the architects the owner picks, each as a
  -- matter of its queue; the engine, which forwards a new one to the architect the owner's setting names
  -- (owner_choices), and hands an architect the notice of a fault forwarded to it.
  fault TEXT,
  -- Whether the notice reminds the owner of the asks waiting for the owner's answer, which the engine does once an
  -- interval of the installation's setting while any waits: 1 for such a reminder, 0 for another notice.
  -- Read by: the engine, which reminds again an interval after the last reminder.
  reminder INTEGER NOT NULL DEFAULT 0 CHECK (reminder IN (0, 1))
) STRICT;

INSERT INTO engine_notices (id, work, text, at, read_at, ask, fault)
SELECT id, work, text, at, read_at, ask, fault
FROM engine_notices_version_48;
DROP TABLE engine_notices_version_48;
