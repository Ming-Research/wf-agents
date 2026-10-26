-- The wf-agents database.
--
-- This file is the only definition of the database's tables and views. The engine creates the database from it, and
-- a change to the database is a change to this file, made in a pull request together with the step that migrates an
-- existing database.
--
-- Inside each CREATE statement, the first comment says what the table holds, why it exists and who reads it; the
-- comment above each column does the same for the column. SQLite keeps these comments in the database, and the engine
-- gives them to agents as the description of the tables they can query, so they are written for agents as much as
-- for the people who maintain this file. A column nobody reads is removed. A table whose description has the line
-- "Hidden from agents." is left out of what agents can read and query.
--
-- Rules every table follows:
-- - Current state is stored. History is written once and never changed. Anything that can be computed from these is
--   a view, not a column.
-- - Who created or changed a row, and when, is in changes, not in the row.
-- - Times are UTC milliseconds since the Unix epoch.
-- - JSON columns hold text that passes json_valid.
-- - Cell, line and transition names are the ones in the flow version of the work they belong to.
-- - Project, machine, level and CLI names are the ones in the configuration. The configuration is not pinned to a
--   work: a committed change applies from the next round on. Each round keeps the input it was actually given and
--   the configuration commit it ran with.

CREATE TABLE items (
  -- A piece of work to do, as its creator wrote it. Text that a work's rounds produce later (the problem as the
  -- engineer understood it, the study, the design, progress) is not stored here: views read it from rounds. Items own
  -- nothing; how they depend on each other is in item_links. Only cancellation is stored as a status; every other
  -- status is derived.
  -- Read by: every cell that reads the item or the items around it; the engine when it starts and stops works.

  -- Item number, one sequence across all projects.
  id INTEGER PRIMARY KEY,
  -- The project the item belongs to. Picks the repository, CI and settings its works run with.
  -- Read by: the engine; cells 2 and 2b, through the project's design goals.
  project TEXT NOT NULL,
  -- What to do, as an action. Read by: cell 1 for the items around this one; people.
  title TEXT NOT NULL,
  -- What cannot be done or is wrong, and why the work is worth doing, as written at creation. The engineer's own
  -- understanding replaces them in views once cell 1 has run; these stay as written.
  -- Read by: cell 1, for this item and the items that depend on it; cell 5 reads the problem only.
  problem TEXT NOT NULL,
  why TEXT NOT NULL,
  -- How to do it, when the creator already knows; NULL otherwise. Read by: cell 1.
  approach TEXT,
  -- What the work hands in: a change to merge, or a report. Read by: cells 1 and 5a; the engine, which runs reports
  -- without full CI on the draft.
  deliver TEXT NOT NULL CHECK (deliver IN ('change', 'report')),
  -- Agent level; decides which agent and model run the work. Read by: the engine at cell 0.
  level TEXT NOT NULL,
  -- The locked acceptance: a JSON list of acceptance rows in the flow's acceptance shape. NULL until a first design
  -- passes its checks. Every later design must hand in exactly this list; only the advisor can change it.
  -- Read by: cells 2a, 4, 5, 5a and 6; the engine, which compares each design's acceptance with it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round whose acceptance was locked: a design round, or an advisor round that changed it.
  -- Read by: cell 2a, to see whether the advisor changed it.
  acceptance_round INTEGER REFERENCES rounds(id),
  -- Why the item was cancelled; NULL while it is not. Read by: the engine; cell 1 for the items around this one.
  cancelled TEXT,
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL))
) STRICT;

CREATE TABLE item_links (
  -- Dependencies between items: item cannot finish until depends_on is done. Together they form a directed acyclic
  -- graph, and the engine refuses a write that would close a cycle. Splitting an item means creating new items and
  -- making it depend on them. An item can start while what it depends on is unfinished; it waits before it merges.
  -- Read by: cell 1 (the items above and beside this one, what it waits for, who waits for it), 3e (what it has been
  -- split into), 11 and 11b (what the items it waits for handed in), 19 (how far they have got).
  item INTEGER NOT NULL REFERENCES items(id),
  depends_on INTEGER NOT NULL REFERENCES items(id),
  -- What item needs depends_on to hand in, in one sentence. Read by: cells 1 and 11b.
  needs TEXT NOT NULL,
  PRIMARY KEY (item, depends_on),
  CHECK (item <> depends_on)
) STRICT;

CREATE TABLE dispatch_queue (
  -- Items waiting for a work to start, in the order the owner sets by hand. When a slot is free the engine starts a
  -- work on the first one. This table goes away once the architect's plan derives the order.
  -- Read by: the engine at cell 0.
  item INTEGER PRIMARY KEY REFERENCES items(id),
  -- Lower starts first. A real number, so an item can be put between two others without renumbering.
  position REAL NOT NULL UNIQUE
) STRICT;

CREATE TABLE cards (
  -- What the owner is asked to look at: design cards to confirm before a merge, and questions an agent puts to the
  -- owner. A card is handed in by one round and belongs to that round; its work and item are found through it. A
  -- ruled card never changes; a decision that has to change becomes a new card that replaces the old one.
  -- Read by: cell 1 (cards ruled on this item and on the items that depend on it), 2 and 2a (rulings that disagreed or
  -- chose another option), 2a (the latest design card of the items that depend on this one), 15 (the design card to
  -- look at), 16 (the question), 19 (whether the design card has been confirmed).
  id INTEGER PRIMARY KEY,
  -- The round that handed the card in.
  round INTEGER NOT NULL REFERENCES rounds(id),
  -- design: a change to the design tree, to confirm before the merge. ask: a question for the owner.
  kind TEXT NOT NULL CHECK (kind IN ('design', 'ask')),
  -- The card as the round handed it in, JSON, in the flow's design card or card shape. Copied from that round's
  -- output, which never changes, so the copy cannot drift.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- open: waiting for a ruling. ruled: everyone it waits for has ruled. replaced: a newer card took its place and it
  -- can no longer be ruled. Read by: the engine; cell 19.
  status TEXT NOT NULL CHECK (status IN ('open', 'ruled', 'replaced')),
  -- The rulings in the order given, JSON list: who ruled (owner or architect), the verdict, the option chosen and
  -- their own words. Read by: cells 1, 2, 2a and 19.
  rulings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(rulings)),
  -- The card that replaced this one. Read by: people following a decision to its latest card.
  replaced_by INTEGER REFERENCES cards(id),
  CHECK ((status = 'replaced') = (replaced_by IS NOT NULL))
) STRICT;

CREATE TABLE flow_versions (
  -- Every compiled flow a work has run on. A work keeps the version it started with until it ends, and a new version
  -- applies only to works started after it, so old rounds stay readable after the flow changes.
  -- Read by: the engine; people and agents reading old rounds.
  id INTEGER PRIMARY KEY,
  -- Hash of compiled, so each version is stored once.
  digest TEXT NOT NULL UNIQUE,
  -- The compiler's expanded flow, JSON: every graph's cells, inputs, return schemas and transition rows.
  compiled TEXT NOT NULL CHECK (json_valid(compiled))
) STRICT;

CREATE TABLE works (
  -- One run of a graph of the flow: an engineer's work on an item, or one wake of the architect. It holds the current
  -- state the engine needs to take the next step, written in the same transaction as the round that changed it; the
  -- history is in rounds. Which cell brought the work to its cell, whether it is done, what it waits for and when it
  -- started and ended are derived from its rounds. At most one work per item runs at a time.
  -- Read by: the engine at every step.
  id INTEGER PRIMARY KEY,
  -- The item worked on; NULL for an architect wake, which belongs to no item.
  item INTEGER REFERENCES items(id),
  flow_version INTEGER NOT NULL REFERENCES flow_versions(id),
  -- Which graph of that version it runs. Read by: the engine.
  graph TEXT NOT NULL CHECK (graph IN ('engineer', 'architect')),
  -- The cell the work is at. Read by: the engine; views of status and slots.
  at TEXT NOT NULL,
  -- The cell the work left to ask or to be answered; the answer resumes it there. NULL when nothing is pending.
  -- Read by: the engine; cell 11, to know where the question was asked.
  resume_at TEXT,
  -- The cells waiting for an answer, innermost last, JSON list. Read by: the engine, to route answers back.
  waiting TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(waiting)),
  -- Values earlier transition rows recorded, JSON object: whether full CI has passed, whether this design has been
  -- pushed, whether a dedicated adversarial test is needed, and the like.
  -- Read by: the engine; cells 2a, 4 and 18 through their inputs.
  recorded TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(recorded)),
  -- The machine every round of the work runs on; set when the work starts. Read by: the engine.
  machine TEXT,
  -- The worktree path on that machine and the branch, created when the work starts and removed when it ends.
  -- Read by: the engine and the session manager on that machine.
  worktree TEXT,
  branch TEXT,
  -- Pull request number; NULL before the first push. Read by: cells 9 and 13; people.
  pr INTEGER,
  -- The head last pushed; the next push is checked against it. Read by: cells 7, 9, 13 and 18.
  pushed_head TEXT,
  -- Why the owner or the architect paused the work; NULL while it runs. Read by: the engine; the slots view.
  paused TEXT,
  -- Why the work was stopped before its end cell, for example because its item was cancelled; NULL otherwise.
  -- Read by: the engine; the status view.
  ended TEXT
) STRICT;

CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line keeps one session for the whole work and lets the CLI compact
  -- it; after a compaction the engine gives the cell's rules and inputs again.
  -- Hidden from agents.
  -- Read by: the engine, to resume a line.
  id INTEGER PRIMARY KEY,
  work INTEGER NOT NULL REFERENCES works(id),
  -- The line, for example engineer, review, adversarial or advisor.
  line TEXT NOT NULL,
  -- The CLI that started the session. Only that CLI can resume it, so this stays when the configuration changes.
  cli TEXT NOT NULL,
  -- The CLI's own id for the session, passed to it to resume.
  cli_session TEXT NOT NULL,
  UNIQUE (work, line)
) STRICT;

CREATE TABLE rounds (
  -- One round: one CLI run of an agent, from the input the engine gives to the return the agent hands in; or one step
  -- the engine takes itself, one run of a tool cell, one answer from the owner. A pause, a question and a request for
  -- tools each end a round. The row is inserted when the round starts, completed when it ends, and never changed after.
  -- Read by: every cell input that names a field another cell returned; the work's history, which cells 5a, 6, 12 and
  -- 15 read; the views of counters, anomalies and time.
  id INTEGER PRIMARY KEY,
  work INTEGER NOT NULL REFERENCES works(id),
  -- The cell; NULL for a round off the graph, in which a line that is not running only answers an interjection.
  cell TEXT,
  -- The session the agent ran in; NULL for the engine's own steps, tool cells and the owner.
  session INTEGER REFERENCES sessions(id),
  -- What the engine gave, JSON: each input and rule as it was given. Read by: the history.
  input TEXT NOT NULL CHECK (json_valid(input)),
  -- The whole return, JSON: for the engine's own step what it computed, for a tool cell the tools' results, for the
  -- owner the answer. NULL while the round runs. Read by: every input that names a returned field.
  output TEXT CHECK (output IS NULL OR json_valid(output)),
  -- The transition row the return took. NULL while the round runs, and when the return was rejected.
  -- Read by: the engine; the views of counters and anomalies.
  transition TEXT,
  -- Why the engine sent the return back or the run failed, JSON: a check it failed, a wrong format, an error, a
  -- timeout. NULL when the return was accepted. Read by: the history; the anomalies view.
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
  CHECK (transition IS NULL OR rejected IS NULL)
) STRICT;

CREATE TABLE tool_runs (
  -- Each attempt of a tool the engine runs: the tools an agent asks for, run at cell 13, and the tool cells 7, 9 and
  -- 10. A retry is a new row.
  -- Read by: the round that asked for the tools (their results); cells 5 and 6 (each measurement); 15 (a tool that
  -- still failed after retries); the history.
  id INTEGER PRIMARY KEY,
  -- The round in which the engine ran the tool.
  round INTEGER NOT NULL REFERENCES rounds(id),
  -- The tool's name in the flow.
  tool TEXT NOT NULL,
  -- The arguments handed in, JSON.
  args TEXT NOT NULL CHECK (json_valid(args)),
  -- 1 for the first attempt, counting up with each retry.
  attempt INTEGER NOT NULL CHECK (attempt >= 1),
  -- The result, JSON, including a refusal and its reason. NULL while it runs.
  result TEXT CHECK (result IS NULL OR json_valid(result)),
  -- Local paths of the logs and artifacts kept, JSON list.
  files TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(files)),
  -- When it was queued, started and ended. Waiting in a machine's queue is kept apart from running.
  -- Read by: the views of time.
  queued_at INTEGER NOT NULL,
  started_at INTEGER,
  ended_at INTEGER
) STRICT;

CREATE TABLE interjections (
  -- What the owner or the architect says to a running work. The engine hands each one to the line it is for: during a
  -- round after the agent's next tool use, or at the front of the next round's input. Rulings on design cards arrive
  -- this way too. Replies are in the output of the round that answered.
  -- Read by: every agent cell; cells 18, 19 and 20 when the work stands at them.
  id INTEGER PRIMARY KEY,
  work INTEGER NOT NULL REFERENCES works(id),
  -- The line it is for.
  line TEXT NOT NULL DEFAULT 'engineer',
  -- Who said it. When the owner and the architect disagree, the owner's word holds.
  speaker TEXT NOT NULL CHECK (speaker IN ('owner', 'architect')),
  -- Their words as said.
  text TEXT NOT NULL,
  -- The round it was handed in; NULL until then. Read by: the engine, to find what is still to hand in.
  delivered_round INTEGER REFERENCES rounds(id)
) STRICT;

CREATE TABLE known_red_checks (
  -- CI checks known to fail for a reason outside any one work, so full CI and the merge queue do not hold them against
  -- the work. Added while works run, which is why this is a table and not configuration. A row is removed when its
  -- check is fixed. How often a check failed now and then is counted from tool_runs.
  -- Read by: cells 7 and 9.
  project TEXT NOT NULL,
  check_name TEXT NOT NULL,
  -- Why it fails.
  why TEXT NOT NULL,
  -- The item that fixes it.
  fix_item INTEGER NOT NULL REFERENCES items(id),
  PRIMARY KEY (project, check_name)
) STRICT;

CREATE TABLE changes (
  -- Every change to a row of a state table: who made it, when, the row before and after. Written by triggers that the
  -- engine generates from this file, in the same transaction as the change, so no write can skip it. rounds and
  -- tool_runs are history themselves and are not tracked; neither are changes and outbox.
  -- Read by: people and agents looking at how something got where it is; views that need when a row was created.
  id INTEGER PRIMARY KEY,
  -- The transaction; the rows one write changed share it.
  tx INTEGER NOT NULL,
  at INTEGER NOT NULL,
  -- Who made the change.
  actor TEXT NOT NULL CHECK (actor IN ('owner', 'architect', 'engine')),
  table_name TEXT NOT NULL,
  -- The changed row's primary key, JSON.
  row_key TEXT NOT NULL CHECK (json_valid(row_key)),
  -- The row before and after, JSON objects. old_row is NULL for an insert, new_row for a delete.
  old_row TEXT CHECK (old_row IS NULL OR json_valid(old_row)),
  new_row TEXT CHECK (new_row IS NULL OR json_valid(new_row)),
  CHECK (old_row IS NOT NULL OR new_row IS NOT NULL)
) STRICT;

CREATE TABLE outbox (
  -- Actions the engine still has to carry out, written in the same transaction as the change that calls for them, so
  -- a crash loses none. A row is deleted once its action is done; what the action did is in rounds or tool_runs. On
  -- start the engine first carries out what is left here.
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

-- Indexes for the reads every step makes.

-- What an item depends on is the primary key; who depends on an item needs this.
CREATE INDEX item_links_by_depends_on ON item_links (depends_on);
-- The works of an item.
CREATE INDEX works_by_item ON works (item);
-- The rounds of a work in order: every input that names a returned field, and the history.
CREATE INDEX rounds_by_work ON rounds (work, id);
-- The tools a round ran.
CREATE INDEX tool_runs_by_round ON tool_runs (round);
-- The cards a round handed in.
CREATE INDEX cards_by_round ON cards (round);
-- Interjections still to hand in to a work.
CREATE INDEX interjections_waiting ON interjections (work) WHERE delivered_round IS NULL;
-- The history of one row.
CREATE INDEX changes_by_row ON changes (table_name, row_key);
-- The next action due.
CREATE INDEX outbox_by_run_after ON outbox (run_after);
