-- Takes a database to one whose rounds keep the findings set aside: the problems of a return taken that only checks
-- the work said it changes found, which the engine keeps on the round instead of sending the return back, until the
-- change to its checks is installed, for the reviews to judge; the new view `work_set_aside` gives them one to a row.
-- The rounds already recorded have none. The rounds are made afresh with their rows, as the new column's description
-- is part of the table's description, which agents read; a migration runs with the foreign keys off, as SQLite makes a
-- table afresh, and every one is checked before the migration commits. Each statement below that defines something is
-- written exactly as db/schema.sql writes it.

CREATE TABLE rounds_before_set_aside AS SELECT * FROM rounds;
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
  -- Why the engine sent the return back or the run failed, JSON. A return sent back holds, under `check` (it failed
  -- the engine's checks) or `format` (it does not fit its cell's schema), the text it was sent back with, and under
  -- `problems` each problem in it, in the order found, as `{check, field, text}`: `check` is the check's name, or
  -- `format` for a place that does not fit the schema; `field` is the check's `at` in the flow
  -- (`columns[pass].evidence`) or, for `format`, the path in the schema with each index of a list written `[]`
  -- (`columns[].evidence`; the text keeps the index), and null for a problem about the return as a whole: the two
  -- kinds of path are told apart by `check`. A return sent back before `problems` was kept has none. A run that ended
  -- without a return holds its kind as the flow names them. NULL when the return was accepted.
  -- Read by: the engine, which counts returns sent back apart from failed runs; the anomalies view; the view
  -- `sent_back_problems`.
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
  -- The problems of a return taken that only checks the work said it changes (`checks_changed`) found, JSON: each as
  -- `rejected` keeps a problem under `problems`, `{check, field, text}`, with its number in the work under `id` (S1,
  -- S2, ...), in the order found. Until a change to the engine's checks is installed, the engine judges the work that
  -- makes it by the checks as they were; what only those checks find is kept here instead of sending the return back,
  -- for the reviews to judge. NULL for a round with none, and for any round that took no row: a return sent back keeps
  -- every problem in `rejected`. Read by: the view `work_set_aside`.
  set_aside TEXT CHECK (set_aside IS NULL OR (json_valid(set_aside) AND transition IS NOT NULL)),
  PRIMARY KEY (work, n),
  CHECK (transition IS NULL OR rejected IS NULL)
) STRICT;

-- The rows keep the order they had, which the rounds' rowids follow.
INSERT INTO rounds (work, n, cell, session, input, output, transition, rejected, config_commit, model, usage,
    compactions, started_at, ended_at, counts)
  SELECT work, n, cell, session, input, output, transition, rejected, config_commit, model, usage,
    compactions, started_at, ended_at, counts
  FROM rounds_before_set_aside;
DROP TABLE rounds_before_set_aside;

CREATE INDEX rounds_counts ON rounds (work, counts) WHERE counts IS NOT NULL;

CREATE VIEW work_set_aside AS
  -- Each finding set aside in each work, one row each, in the order found (`position` counts from 0 in its round): the
  -- round whose return was taken despite it, its number in the work (`id`, S1, S2, ...), the check that found it
  -- (`check_name`), the field and the text, as `rounds.set_aside` keeps them. A finding is judged once a round of the
  -- details review (`T3`) or the re-check (`T4`) after it handed in its answer, which the engine works out. Read by: the
  -- engine, which numbers each new one after those before, gives the reviews and the merge gate those no review judged
  -- (the input `set_aside`), and has the merge gate send those to the fix.
  SELECT r.work, r.n AS round, r.cell, p.key AS position,
    json_extract(p.value, '$.id') AS id,
    json_extract(p.value, '$.check') AS check_name,
    json_extract(p.value, '$.field') AS field,
    json_extract(p.value, '$.text') AS text
  FROM rounds r, json_each(r.set_aside) p;
