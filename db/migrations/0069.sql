-- Takes a version 68 database to version 69: each round keeps the counter the row it took counts (rounds.counts), and
-- the views of counters read it from there, not from the compiled flow, which they parsed again for every round. The
-- rounds already written get it by the rule the views had: the row each round took (view round_rows), named by the
-- round's cell, in the graph of the work, in the flow version the work is on now. A flow version whose rows could not
-- be told apart by that rule makes the migration fail. The rounds are made afresh with their rows, as the definition of
-- the new column is part of their description, which agents read; a migration runs with the foreign keys off, as SQLite
-- makes a table afresh, and every one is checked before the migration commits. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

-- The rows that count something, in the flow versions works are on, read from each compiled flow once. Two rows of one
-- cell with the same name that both count something fail the primary key: the old views counted such a round twice,
-- which one column cannot hold.
CREATE TEMP TABLE counted_rows (
  version INTEGER NOT NULL,
  graph TEXT NOT NULL,
  cell TEXT NOT NULL,
  name TEXT NOT NULL,
  counts TEXT NOT NULL,
  PRIMARY KEY (version, graph, cell, name)
) WITHOUT ROWID;
INSERT INTO temp.counted_rows
  SELECT v.id, json_extract(g.value, '$.name'), json_extract(c.value, '$.id'), json_extract(x.value, '$.name'),
    json_extract(x.value, '$.counts')
  FROM flow_versions v
  JOIN json_each(v.compiled, '$.graphs') g
  JOIN json_each(g.value, '$.cells') c
  JOIN json_each(c.value, '$.rows') x
  WHERE v.id IN (SELECT flow_version FROM works)
    AND json_extract(g.value, '$.name') IS NOT NULL AND json_extract(c.value, '$.id') IS NOT NULL
    AND json_extract(x.value, '$.name') IS NOT NULL AND json_extract(x.value, '$.counts') IS NOT NULL;

-- The counter of the row each round took, read before the rounds are made afresh, since the view of the rows reads them.
CREATE TEMP TABLE round_counts AS
  SELECT x.work, x.round, k.counts
  FROM round_rows x
  JOIN works w ON w.id = x.work
  JOIN temp.counted_rows k ON k.version = w.flow_version AND k.graph = w.graph AND k.cell = x.cell AND k.name = x.took;

DROP VIEW work_asking;
DROP VIEW work_counters;

CREATE TABLE rounds_version_68 AS SELECT * FROM rounds;
DROP TABLE rounds;

CREATE TABLE rounds (
  -- One round: one CLI run of an agent, from the message the engine sends to the return the agent hands in, or to a
  -- run that ends without one; or one step the engine takes itself, one run of a tool cell, one answer from the owner.
  -- A pause, a question and a request for tools each end a round. The row is inserted when the round starts,
  -- completed when it ends, and never changed after. A round is known by its work and its number in the work, and
  -- named R<work>-<n>: R5-9 is the ninth round of work 5.
  -- Read by: every cell input that names a field another cell returned; the engine, to tell what each session already
  -- holds; the views of counters, anomalies and time; the owner's history command.
  -- The work, and the round's number in it: the work's rounds are counted from 1 in the order they started.
  work INTEGER NOT NULL REFERENCES works(id),
  n INTEGER NOT NULL CHECK (n >= 1),
  -- The cell; NULL for a round off the graph, in which a line that is not running only answers an interjection.
  cell TEXT,
  -- The session the agent ran in; NULL for the engine's own steps, tool cells and the owner.
  session INTEGER REFERENCES sessions(id),
  -- What the engine gave, JSON. For an agent's round: the message, word for word; each conditional input in it, by its
  -- address and the hash of its content, and whether it was given or only referred to; and the call (the whole
  -- command line, the session it started or resumed, the engine's version and the command line's). For another step,
  -- the values of its inputs; for cell `start`, nothing. Rounds recorded before this format keep the prompt and inputs.
  -- Read by: the engine, to tell what a session already holds; the owner's interface.
  input TEXT NOT NULL CHECK (json_valid(input)),
  -- The whole return, JSON: for the engine's own step what it computed, for a tool cell the tools' results, for the
  -- owner the answer. NULL while the round runs. Read by: every input that names a returned field.
  output TEXT CHECK (output IS NULL OR json_valid(output)),
  -- The transition row the return took. NULL while the round runs, and when the return was rejected.
  -- Read by: the engine; the view of anomalies.
  transition TEXT,
  -- Why the engine sent the return back or the run failed, JSON: a check it failed or a wrong format; or a run that
  -- ended without a return, with its kind as the flow names them. NULL when the return was accepted.
  -- Read by: the engine, which counts returns sent back apart from failed runs; the anomalies view.
  rejected TEXT CHECK (rejected IS NULL OR json_valid(rejected)),
  -- The commit of the configuration the round ran with. The engine reads only committed configuration, so this
  -- names every setting behind the round, including those that shaped the engine's own decisions and never reached
  -- an agent's input. Read by: people and agents tracing why a round went the way it did.
  config_commit TEXT NOT NULL,
  -- The model the round ran on; NULL when no model ran. Recorded each round because the configuration can change.
  model TEXT,
  -- Tokens and cost as the CLI reported them, JSON. Read by: the views of cost.
  usage TEXT CHECK (usage IS NULL OR json_valid(usage)),
  -- How many times the CLI compacted the session during the round. Read by: the anomalies view.
  compactions INTEGER NOT NULL DEFAULT 0 CHECK (compactions >= 0),
  -- When the round started and ended. Read by: the views of time.
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  -- The counter the row this round took counts, as the flow version the work ran on said it when the round ended; the
  -- row is the one the return took (`transition`) or, for a round that ended in no return its cell takes, the shared
  -- row the engine took for it (view `round_rows`). NULL when the round took no row, or the row counts nothing. A
  -- round's row can be found again in the flow only by the version it ran on, which a round does not record and a
  -- work leaves when it moves to a newer one, so the counter is kept with the round, written in the same step as
  -- `transition` or `rejected`. 'total_time' is kept as the row says it, though the views count no round for it.
  -- Read by: the views of counters and of how many times a work asked, through the index `rounds_counts`.
  counts TEXT CHECK (counts IS NULL OR transition IS NOT NULL OR rejected IS NOT NULL),
  PRIMARY KEY (work, n),
  CHECK (transition IS NULL OR rejected IS NULL)
) STRICT;

-- The rows keep the order they had, which the rounds' rowids follow.
INSERT INTO rounds (work, n, cell, session, input, output, transition, rejected, config_commit, model, usage,
    compactions, started_at, ended_at, counts)
  SELECT r.work, r.n, r.cell, r.session, r.input, r.output, r.transition, r.rejected, r.config_commit, r.model, r.usage,
    r.compactions, r.started_at, r.ended_at, k.counts
  FROM rounds_version_68 r
  LEFT JOIN temp.round_counts k ON k.work = r.work AND k.round = r.n;
DROP TABLE rounds_version_68;
DROP TABLE temp.round_counts;
DROP TABLE temp.counted_rows;

CREATE INDEX rounds_counts ON rounds (work, counts) WHERE counts IS NOT NULL;

CREATE VIEW work_counters AS
  -- How many times each work took the rows that count each counter, by the counter each round recorded when it ended
  -- (`rounds.counts`); and its total time, in minutes, which no row counts: the time of its agents' rounds and of the
  -- tools they had the engine run (cell `X1`). Read by: cell `A1`; the engine, against the project's limits; the
  -- owner's interface.
  SELECT r.work, r.counts AS counter, count(*) AS times
  FROM rounds r
  WHERE r.counts IS NOT NULL AND r.counts <> 'total_time'
  GROUP BY r.work, r.counts
  UNION ALL
  SELECT r.work, 'total_time' AS counter, sum(r.ended_at - r.started_at) / 60000 AS times
  FROM rounds r
  WHERE r.ended_at IS NOT NULL AND (r.session IS NOT NULL OR r.cell IN ('X1', '13', 'tools', 'TOOLS'))
  GROUP BY r.work;

CREATE VIEW work_asking AS
  -- The cell that asked, or that took up an interjection, and how many times each work has asked: what choosing the
  -- way on needs of where the work stands. Read by: cell `A4`.
  SELECT w.id AS work, w.resume_at AS asked_at,
    (SELECT count(*) FROM rounds r WHERE r.work = w.id AND r.counts = 'asks') AS asks
  FROM works w;
