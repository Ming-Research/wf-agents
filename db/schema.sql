-- The wf-agents database.
--
-- This file is the only definition of the database's tables and views. `wf-agents init` creates the database from it,
-- and a change to the database is a change to this file, made in a pull request together with the step that migrates
-- an existing database.
--
-- Inside each CREATE statement, the first comment says what the table holds, why it exists and who reads it; the
-- comment above each column does the same for the column. SQLite keeps these comments in the database. The engine
-- gives agents an index of the tables and views, each with the first line of its first comment, and an agent reads a
-- definition in full, its comments included, through the query tool; so they are written for agents as much as for
-- the people who maintain this file. A column nobody reads is removed. A table whose description has the line
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
  -- The round whose acceptance was locked, by its work and its number in the work: a design round, or an advisor
  -- round that changed it. Read by: cell 2a, to see whether the advisor changed it.
  acceptance_work INTEGER,
  acceptance_round INTEGER,
  -- Why the item was cancelled; NULL while it is not. Read by: the engine; cell 1 for the items around this one.
  cancelled TEXT,
  -- For an item imported from outside this flow, such as an item of an earlier board: where it came from, as the
  -- import wrote it; NULL for an item made here. Read by: cell 1, through what already exists for the item; the
  -- owner's interface.
  imported_from TEXT,
  -- The branch and the pull request that work on the item had outside this flow, as the import recorded them; NULL
  -- when there were none. Read by: cell 0, which takes them over when the item had no work here before; cell 1; the
  -- owner's interface.
  imported_branch TEXT,
  imported_pr INTEGER,
  -- The plan segment a root item hangs on: an item no other item depends on hangs on a segment, or on none, which
  -- puts it in the pool no work starts from; an item others depend on goes with them. Set by the architect's
  -- approved changes. Read by: the dispatch order in ready_items; the architect through plan.
  segment TEXT REFERENCES plan_segments(key),
  -- What the item's work will touch that a plan gate governs (specification, public_interface, data_format), JSON
  -- list, as the architect wrote it when creating the item; NULL for an item made otherwise.
  -- Read by: the architect, which checks the segment it hangs on may touch them.
  touches TEXT CHECK (touches IS NULL OR json_valid(touches)),
  -- The acceptance as the creator wrote it, JSON list of conditions; NULL when the creator wrote none. The locked
  -- acceptance, designed by the work, is acceptance. Read by: cell 1, through item_body.
  wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted)),
  -- Why the architect holds the item, with the owner's approval: no work starts on it while it is held; NULL while it
  -- is not. Read by: ready_items; the architect.
  held TEXT,
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
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

CREATE TABLE design_cards (
  -- A change to the design tree for the owner to confirm before it is merged: each decision the change adds, changes
  -- or removes. It asks the owner nothing; what the owner is asked is an ask of the owner. A design card is handed in
  -- by one round and belongs to that round; its item is found through the round's work. A ruled design card never
  -- changes; a decision that has to change comes in a new design card that replaces the old one.
  -- Read by: cell 1 (design cards ruled on this item), 2 and 2a (rulings that disagreed or chose another option), 2a
  -- (the latest design card of the items that depend on this one), 5a and 5b (the work's design card as it stands),
  -- 15 (the design card to look at), 19 (whether the design card has been confirmed).
  id INTEGER PRIMARY KEY,
  -- The round that handed the design card in: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The design card as the round handed it in, JSON, in the flow's design card shape. Copied from that round's
  -- output, which never changes, so the copy cannot drift.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- open: waiting for a ruling. ruled: everyone it waits for has ruled. replaced: a newer design card took its place
  -- and it can no longer be ruled. Read by: the engine; cell 19.
  status TEXT NOT NULL CHECK (status IN ('open', 'ruled', 'replaced')),
  -- The rulings in the order given, JSON list, the owner's and the architect's alike: who ruled and when, and a
  -- verdict on each decision of the card by its number (confirm or disagree), with any approach chosen instead and
  -- why; a ruling made before rulings were per decision holds one verdict on the whole card, its option and words.
  -- The card is confirmed when a ruling confirms every decision. Read by: cells 1, 2, 2a and 19.
  rulings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(rulings)),
  -- The design card that replaced this one. Read by: people following a decision to its latest design card.
  replaced_by INTEGER REFERENCES design_cards(id),
  -- For a design card that replaces earlier ones of its work: what changed from them and why, in the owner's
  -- language, as the round that handed it in wrote it; NULL for a work's first design card, and for one handed in
  -- before the flow asked for it. Read by: the owner's interface, beside each card it replaced.
  changed TEXT,
  -- A replaced design card names its successor, unless its work ended without one.
  CHECK (replaced_by IS NULL OR status = 'replaced'),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE owner_asks (
  -- An ask of the owner: what an agent asks the owner, or asks the architect while the owner answers for the
  -- architect. The work waits for the answer, which goes back unchanged to the cell that asked. It belongs to the
  -- round of the asking cell that put it to the owner; its item is found through the round's work.
  -- Read by: cell 1 (the asks of the owner on this item, with their answers); the engine, which waits for the answer.
  id INTEGER PRIMARY KEY,
  -- The round that put it to the owner: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- What is asked, JSON: whom it asks, the work and item, and what the asking cell handed in. Copied from what that
  -- round was given, which never changes.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The owner's answer, JSON: who answered and when, and the text; for an ask of the architect, the rest of the
  -- architect's reply too. NULL while it waits. Read by: the engine; cell 1.
  answer TEXT CHECK (answer IS NULL OR json_valid(answer)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE owner_messages (
  -- What an agent tells the owner in a return, one row for each message: something the owner should know that needs
  -- no decision and no answer. The work goes on. It belongs to the round whose return holds it; its item is found
  -- through the round's work. Read by: the owner's interface, which shows those unread.
  id INTEGER PRIMARY KEY,
  -- The round whose return holds it: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The message, copied from that round's return, which never changes.
  text TEXT NOT NULL,
  -- When the owner marked it read; NULL while it is unread. Read by: the owner's interface.
  read_at INTEGER,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE engine_notices (
  -- What the engine tells the owner of its own accord, one row for each notice: a work stopped by a new flow version,
  -- a design card posted, every work halted until a usage limit resets, and the like. It needs no answer. The same text
  -- goes to the notifications log.
  -- Hidden from agents.
  -- Read by: the owner's interface, which shows those unread with the messages agents wrote.
  id INTEGER PRIMARY KEY,
  -- The work it is about; NULL for one about no work.
  work INTEGER REFERENCES works(id),
  -- The notice, as the engine wrote it.
  text TEXT NOT NULL,
  -- When the engine wrote it.
  at INTEGER NOT NULL,
  -- When the owner marked it read; NULL while it is unread. Read by: the owner's interface.
  read_at INTEGER,
  -- When the notice is of a new ask of the owner: that ask, for the owner's interface to link to. Read by: the
  -- owner's interface.
  ask INTEGER REFERENCES owner_asks(id)
) STRICT;

CREATE TABLE contents (
  -- Each large text a round's record holds, once: a conditional input given in a message, a cell's task, the
  -- structured-output schema of a command line. The record refers to it by the SHA-256 of the text, as
  -- {"content": hash}, and the text it was is that record with each reference put back. Written once, never changed.
  -- Read by: the engine and the owner's interface, putting a round's record back together.
  hash TEXT PRIMARY KEY,
  text TEXT NOT NULL
) STRICT;

CREATE TABLE flow_versions (
  -- Every compiled flow a work has run on. A work runs on the version it started with; when the flow moves to a new
  -- version, the works still running on an older one stop, and their items start again on the new one. Old rounds
  -- stay readable after the flow changes.
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
  -- Why the work is paused: the engine's reason, when a step could not be taken or returns were sent back too often,
  -- or the owner's words; NULL while it goes on. Read by: the engine; the slots view.
  paused TEXT,
  -- Why the work was stopped before its end cell, for example because its item was cancelled; NULL otherwise.
  -- Read by: the engine; the status view.
  ended TEXT,
  -- Who paused the work, the owner or the engine; NULL while it goes on. The owner's Start all lets go on only the
  -- works the owner paused: a work the engine paused waits for the owner to look at it and let it go on by itself.
  -- Read by: the engine, when the owner lets works go on; the owner's interface.
  paused_by TEXT CHECK (paused_by IN ('owner', 'engine'))
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
  -- the values of its inputs; for cell 0, nothing. Rounds recorded before this format keep the prompt and inputs.
  -- Read by: the engine, to tell what a session already holds; the owner's interface.
  input TEXT NOT NULL CHECK (json_valid(input)),
  -- The whole return, JSON: for the engine's own step what it computed, for a tool cell the tools' results, for the
  -- owner the answer. NULL while the round runs. Read by: every input that names a returned field.
  output TEXT CHECK (output IS NULL OR json_valid(output)),
  -- The transition row the return took. NULL while the round runs, and when the return was rejected.
  -- Read by: the engine; the views of counters and anomalies.
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
  PRIMARY KEY (work, n),
  CHECK (transition IS NULL OR rejected IS NULL)
) STRICT;

CREATE TABLE tool_runs (
  -- Each attempt of a tool the engine runs: the tools an agent asks for, run at cell 13, and the tool cells 7, 9 and
  -- 10. A retry is a new row.
  -- Read by: the round that asked for the tools (their results); cells 5 and 6 (each measurement); 5a and 5b (the
  -- latest CI on the work); 15 (a tool that still failed after retries); the history.
  id INTEGER PRIMARY KEY,
  -- The round in which the engine ran the tool: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
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
  ended_at INTEGER,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
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
  -- The round of the work it was handed in, by its number in the work; NULL until then. Read by: the engine, to find
  -- what is still to hand in.
  delivered_round INTEGER,
  FOREIGN KEY (work, delivered_round) REFERENCES rounds (work, n)
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

CREATE TABLE architect_matters (
  -- A matter for the architect: an engineer's question, the owner's words or answer, the owner's approval of the
  -- changes the architect collected, a design card to look at, a tool that failed, a side report, a place the fifth
  -- question found to change, an advance the engine asks for, a next look the architect set, or a new policy. The
  -- architect's queue is the matters due and not handled; its sixth question picks among them by the constitution and
  -- the policy, not by the order they came. A matter that waits wakes the architect.
  -- Read by: the engine, which wakes the architect, hands it each matter's material and takes its replies back; the
  -- architect's cells through architect_queue and wake_handled; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- Its kind, one of the flow's arch_about values.
  kind TEXT NOT NULL,
  -- What it is, in one sentence, as the engine wrote it.
  what TEXT NOT NULL,
  -- Its material, JSON: what the asking cell handed in, the owner's words, what the owner approved, the tool that
  -- failed, the report, the place to change, the next look. Read by: the engine, for the matter's material.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The item it concerns; NULL when it concerns none.
  item INTEGER REFERENCES items(id),
  -- The round that raised it: an engineer's round at the architect's cell, a round that handed in a side report, the
  -- round that handed in a design card, the architect's own round that queued it; NULL for one the owner or the engine
  -- raised.
  work INTEGER,
  round INTEGER,
  -- When it came. Read by: the queue, for how long it waited.
  at INTEGER NOT NULL,
  -- For a next look the architect set for a time, that time; for a side report that is not urgent, an hour after it
  -- came; NULL for a matter due at once. A matter not due wakes no one and is not in the queue.
  due INTEGER,
  -- For a next look the architect set on an item's work ending: that item; the matter is due once the item's latest
  -- work has ended or reached its end. NULL for any other.
  due_item INTEGER REFERENCES items(id),
  -- The held matter an owner's answer is for: a matter of the kind owner_words that answers an ask the architect held
  -- a matter with. NULL for any other. Read by: the architect, through the matter's material.
  answers INTEGER REFERENCES architect_matters(id),
  -- Why the matter waits though picked: owner_ask while an ask of the owner the eighth question put holds it, approval
  -- while changes the architect collected for it wait for the owner's approval; NULL while it does not.
  held TEXT CHECK (held IN ('owner_ask', 'approval')),
  -- The ask of the owner that holds it. Read by: the engine, which queues the owner's answer as a matter of its own.
  held_by INTEGER REFERENCES owner_asks(id),
  -- The architect's reply to an engineer's question, as engine_op reply handed it in, JSON; NULL until it replied.
  -- Read by: the engine, which takes it back unchanged to the work that asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- The architect's round that handled it: its wake and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((held IS NULL) = (held_by IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE dispatch_holds (
  -- What stops new works from starting, one row for each: the architect's engine_op hold_dispatch, the architect
  -- setting the whole straight, the architect setting a new policy. While any row is here, the engine starts no new
  -- work from ready_items; works already running go on.
  -- Read by: the engine at cell 0; the architect, through running_and_next; the owner's interface.
  -- architect: held by engine_op. tidying: the fifth question answered set_straight. policy: a new policy is being set.
  reason TEXT PRIMARY KEY CHECK (reason IN ('architect', 'tidying', 'policy')),
  -- When it began.
  since INTEGER NOT NULL
) STRICT;

CREATE TABLE plan_segments (
  -- A segment of the plan: one piece of delivery under a stage of the policy, the top of a project's items. Root
  -- items hang on segments; the dispatch order comes from the segments' order, the items' dependencies and the slots.
  -- Each project has a standing maintenance segment for defects of committed behaviour. Written only by the
  -- architect's changes the owner approved.
  -- Read by: the architect, through plan; ready_items; cell 19a, through the plan segment the work hangs on.
  -- The segment's key, as the architect named it.
  key TEXT PRIMARY KEY,
  project TEXT NOT NULL,
  -- What it delivers, as an action.
  title TEXT NOT NULL,
  -- The stage of the policy it belongs to, by the stage's number, or maintenance.
  stage TEXT NOT NULL,
  delivers TEXT NOT NULL,
  -- When it counts as done: its acceptance, something that can be checked.
  done_when TEXT NOT NULL,
  -- The segments that must be done first, JSON list of keys.
  after TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(after)),
  -- What it rests on, JSON list of {holds, wrong_if}.
  assumptions TEXT NOT NULL CHECK (json_valid(assumptions)),
  -- What to watch, JSON list of indicators with their lines.
  indicators TEXT NOT NULL CHECK (json_valid(indicators)),
  -- Roughly how large.
  size TEXT NOT NULL,
  -- Until when or what it is deferred; NULL while it is not. The works of its items pause while it is deferred.
  deferred TEXT,
  -- Its place in the order of the segments not deferred, lower first; NULL for one after those ordered.
  position REAL
) STRICT;

CREATE TABLE plan_gates (
  -- A gate of the plan: which segments may touch one thing a gate governs (the specification, a public interface, a
  -- data format); an item of another segment that touches it may not merge. Written only by the architect's changes
  -- the owner approved. Read by: the architect, through plan; cell 19a.
  name TEXT PRIMARY KEY,
  -- The article of the constitution and the point of the policy it rests on.
  basis TEXT NOT NULL,
  -- What it governs: specification, public_interface or data_format.
  what TEXT NOT NULL CHECK (what IN ('specification', 'public_interface', 'data_format')),
  -- The segments that may touch it, JSON list of keys.
  segments TEXT NOT NULL CHECK (json_valid(segments))
) STRICT;

CREATE TABLE plan_budgets (
  -- A budget of the plan: a number the engine measures on the commit to merge, with a line shared out among the
  -- segments; an item past its segment's share may not merge. Written only by the architect's changes the owner
  -- approved. Read by: the architect, through plan; cell 19a.
  name TEXT PRIMARY KEY,
  -- The article of the constitution and the point of the policy it rests on.
  basis TEXT NOT NULL,
  -- What is measured and how.
  measure TEXT NOT NULL,
  -- The whole line.
  line TEXT NOT NULL,
  -- The line shared out, JSON list of {segment, amount}; a segment not listed has none.
  split TEXT NOT NULL CHECK (json_valid(split))
) STRICT;

-- Indexes for the reads every step makes.

-- What an item depends on is the primary key; who depends on an item needs this.
CREATE INDEX item_links_by_depends_on ON item_links (depends_on);
-- The works of an item.
CREATE INDEX works_by_item ON works (item);
-- The rounds of a work in order, which every input that names a returned field and the history read, are the
-- primary key of rounds.
-- The tools a round ran.
CREATE INDEX tool_runs_by_round ON tool_runs (work, round);
-- The design cards a round handed in.
CREATE INDEX design_cards_by_round ON design_cards (work, round);
-- The ask of the owner a round put.
CREATE INDEX owner_asks_by_round ON owner_asks (work, round);
-- The messages to the owner still unread.
CREATE INDEX owner_messages_unread ON owner_messages (id) WHERE read_at IS NULL;
-- Interjections still to hand in to a work.
CREATE INDEX interjections_waiting ON interjections (work) WHERE delivered_round IS NULL;
-- The history of one row.
CREATE INDEX changes_by_row ON changes (table_name, row_key);
-- The next action due.
CREATE INDEX outbox_by_run_after ON outbox (run_after);
-- The architect's matters not handled.
CREATE INDEX architect_matters_waiting ON architect_matters (id) WHERE handled_round IS NULL;

-- Views: what cells read and the owner's interface shows, computed from the tables. A view a cell reads for the item,
-- work or project at hand has a column of that name; the engine filters on it. Cell numbers are those of the
-- engineer's graph.

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: first those the owner queued, in the owner's order; then those
  -- the plan gives, by the order of the segments they hang on, among the segments not deferred whose earlier segments
  -- are done, an item others depend on before them. An item is ready when it is not cancelled, not held, not done and
  -- has no work running. An item leaves the owner's queue when a work starts on it, and may be queued again once its
  -- works ended. A work runs until it ends or its last round, at the cell it is at, completed without taking a row,
  -- which only the end of its graph does; it is done at the end of the engineer's graph. While dispatch_holds has a
  -- row, the engine starts none. Read by: cell 0; the architect, through what runs and starts next.
  WITH RECURSIVE
  running(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.graph = 'engineer' AND w.at = '20'),
  hung(item, segment, depth) AS (
    SELECT i.id, i.segment, 0 FROM items i WHERE i.segment IS NOT NULL
    UNION
    SELECT l.depends_on, h.segment, h.depth + 1 FROM hung h JOIN item_links l ON l.item = h.item),
  open_segments(key, position) AS (
    SELECT s.key, s.position FROM plan_segments s
    WHERE s.deferred IS NULL AND NOT EXISTS (
      SELECT 1 FROM json_each(s.after) a JOIN items i ON i.segment = a.value
      WHERE i.cancelled IS NULL AND i.id NOT IN (SELECT item FROM done)))
  SELECT item, project, title, level, position, segment FROM (
    SELECT i.id AS item, i.project, i.title, i.level, q.position, NULL AS segment, 0 AS src, q.position AS k1,
      0 AS k2, i.id AS k3
    FROM dispatch_queue q JOIN items i ON i.id = q.item
    WHERE i.cancelled IS NULL AND i.held IS NULL AND i.id NOT IN (SELECT item FROM running)
    UNION ALL
    SELECT i.id, i.project, i.title, i.level, NULL, h.segment, 1, coalesce(o.position, 1e18), -max(h.depth), i.id
    FROM hung h JOIN open_segments o ON o.key = h.segment JOIN items i ON i.id = h.item
    WHERE i.cancelled IS NULL AND i.held IS NULL AND i.id NOT IN (SELECT item FROM running)
      AND i.id NOT IN (SELECT item FROM done) AND i.id NOT IN (SELECT item FROM dispatch_queue)
    GROUP BY i.id)
  ORDER BY src, k1, k2, k3;

CREATE VIEW item_body AS
  -- The item as its creator wrote it, the acceptance it wanted included. Read by: cells 1 and 5a.
  SELECT id AS item, project, title, problem, why, approach, deliver, wanted FROM items;

CREATE VIEW item_problem AS
  -- The item's problem: the engineer's understanding once cell 1 has run, as written before.
  -- Read by: cell 5, which is given the problem and the acceptance only.
  SELECT i.id AS item, coalesce((
    SELECT json_extract(r.output, '$.outcome.understanding.problem')
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = i.id AND r.cell = '1' AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.understanding') IS NOT NULL
    ORDER BY r.work DESC, r.n DESC LIMIT 1), i.problem) AS problem
  FROM items i;

CREATE VIEW item_acceptance AS
  -- The item's locked acceptance, and the round that locked it, by its work and its number in the work; NULL until a
  -- first design passes its checks. Read by: cells 2a, 4, 5, 5a and 6.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;

CREATE VIEW item_design_cards AS
  -- Every design card of the item's works, with its rulings. Read by: cells 1, 2, 2a and 19.
  SELECT w.item, c.work, c.id AS design_card, c.round, c.body, c.status, c.rulings, c.replaced_by
  FROM design_cards c JOIN works w ON w.id = c.work;

CREATE VIEW item_asks AS
  -- Every ask of the owner of the item's works, with its answer. Read by: cell 1.
  SELECT w.item, a.work, a.id AS ask, a.round, a.body, a.answer
  FROM owner_asks a JOIN works w ON w.id = a.work;

CREATE VIEW parent_design_cards AS
  -- The design cards of the items that depend on this one, not yet replaced: what a split-off item's parent
  -- decided. Read by: cell 2a.
  SELECT l.depends_on AS item, l.item AS parent, c.id AS design_card, c.body, c.status, c.rulings
  FROM item_links l
  JOIN works w ON w.item = l.item
  JOIN design_cards c ON c.work = w.id
  WHERE c.status <> 'replaced';

CREATE VIEW item_dependencies AS
  -- What the item depends on, with what it needs of each and how far each is: the cell its latest work is at, or
  -- why it was cancelled. Read by: cells 3e and 19.
  SELECT l.item, l.depends_on, l.needs, d.title, d.cancelled,
    (SELECT w.at FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS at,
    (SELECT w.pr FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS pr
  FROM item_links l JOIN items d ON d.id = l.depends_on;

CREATE VIEW item_context AS
  -- Where the item sits in the whole: the items above it (those that depend on it, up to the root), beside it
  -- (the other items its parents depend on) and below it (those it depends on), each with its problem and why.
  -- Read by: cell 1.
  WITH RECURSIVE up(item, other, depth) AS (
    SELECT depends_on, item, 1 FROM item_links
    UNION
    SELECT up.item, l.item, up.depth + 1 FROM up JOIN item_links l ON l.depends_on = up.other
  )
  SELECT up.item, 'above' AS relation, up.depth, o.id AS other, o.title, o.problem, o.why, o.cancelled
  FROM up JOIN items o ON o.id = up.other
  UNION ALL
  SELECT a.depends_on, 'beside', 1, o.id, o.title, o.problem, o.why, o.cancelled
  FROM item_links a JOIN item_links b ON b.item = a.item AND b.depends_on <> a.depends_on
  JOIN items o ON o.id = b.depends_on
  UNION ALL
  SELECT l.item, 'below', 1, o.id, o.title, o.problem, o.why, o.cancelled
  FROM item_links l JOIN items o ON o.id = l.depends_on;

CREATE VIEW work_state AS
  -- Where each work stands, as the engine keeps it. Read by: cells 7, 9, 11a, 13, 18 and 20.
  SELECT id AS work, item, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head,
    paused, ended
  FROM works;

CREATE VIEW work_history AS
  -- Every round of each work in order: the cell, what the engine gave, the return, the row taken or why the
  -- return was sent back, and the tools it ran. Read by: the owner's history command.
  SELECT r.work, r.n AS round, r.cell, r.input, r.output, r.transition, r.rejected, r.started_at, r.ended_at,
    (SELECT json_group_array(json_object('tool', t.tool, 'attempt', t.attempt, 'result', json(t.result)))
     FROM tool_runs t WHERE t.work = r.work AND t.round = r.n) AS tools
  FROM rounds r;

CREATE VIEW work_counters AS
  -- How many times each work took the rows that count each counter, by the compiled flow it runs on.
  -- Read by: cell 12; cell 11, through work_asking; the owner's interface.
  SELECT r.work, json_extract(row.value, '$.counts') AS counter, count(*) AS times
  FROM rounds r
  JOIN works w ON w.id = r.work
  JOIN flow_versions v ON v.id = w.flow_version
  JOIN json_each(v.compiled, '$.graphs') g ON json_extract(g.value, '$.name') = w.graph
  JOIN json_each(g.value, '$.cells') c ON json_extract(c.value, '$.id') = r.cell
  JOIN json_each(c.value, '$.rows') row ON json_extract(row.value, '$.name') = r.transition
  WHERE json_extract(row.value, '$.counts') IS NOT NULL
  GROUP BY r.work, counter;

CREATE VIEW work_asking AS
  -- The cell that asked, or that took up an interjection, and how many times each work has asked: what choosing the
  -- way on needs of where the work stands. Read by: cell 11.
  SELECT w.id AS work, w.resume_at AS asked_at,
    coalesce((SELECT c.times FROM work_counters c WHERE c.work = w.id AND c.counter = 'asks'), 0) AS asks
  FROM works w;

CREATE VIEW work_design_card AS
  -- Each work's design card as it stands: its latest one, unless that was replaced, with its rulings and the numbers
  -- of the earlier design cards of the work, which it replaced. Read by: cells 5a and 5b.
  SELECT c.work, c.id AS design_card, c.round, c.body, c.status, c.rulings,
    (SELECT json_group_array(o.id) FROM design_cards o WHERE o.work = c.work AND o.id < c.id) AS replaces
  FROM design_cards c
  WHERE c.status <> 'replaced' AND c.id = (SELECT max(o.id) FROM design_cards o WHERE o.work = c.work);

CREATE VIEW work_ci AS
  -- The CI on each work: the latest run of the fast CI (the tool ci), of the full CI on the draft and of the tests on
  -- the code before the change (the tool ci_before), each with the head it ran on and its result, which holds the
  -- links to its runs; and the head last pushed. Read by: cells 5a and 5b.
  SELECT w.id AS work, w.pushed_head AS head,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'ci' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS fast_ci,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'full_ci' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS full_ci,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'ci_before' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS ci_before
  FROM works w;

CREATE VIEW work_measurements AS
  -- Every run of the measure tool for each work, with its result and the files kept. Read by: cells 5 and 6.
  SELECT t.work, t.round, t.attempt, t.args, t.result, t.files
  FROM tool_runs t
  WHERE t.tool = 'measure';

CREATE VIEW work_review_record AS
  -- What decides what runs next after a review: the values rows recorded, and the last return of the table review
  -- and of adversarial testing. Read by: cell 18.
  SELECT w.id AS work, w.recorded,
    (SELECT r.output FROM rounds r WHERE r.work = w.id AND r.cell = '5a' AND r.transition IS NOT NULL
     ORDER BY r.n DESC LIMIT 1) AS last_review,
    (SELECT r.output FROM rounds r WHERE r.work = w.id AND r.cell = '6' AND r.transition IS NOT NULL
     ORDER BY r.n DESC LIMIT 1) AS last_adversarial
  FROM works w;

CREATE VIEW waited_items AS
  -- The items each work's item depends on, with what it needs of each, how far each is and, once merged, the
  -- last return of its merge. Read by: cells 11 and 11b.
  SELECT w.id AS work, d.id AS item, l.needs, d.title, d.cancelled,
    (SELECT x.at FROM works x WHERE x.item = d.id ORDER BY x.id DESC LIMIT 1) AS at,
    (SELECT r.output FROM rounds r JOIN works x ON x.id = r.work
     WHERE x.item = d.id AND r.cell = '9' AND r.transition = 'merged' ORDER BY r.work DESC, r.n DESC LIMIT 1) AS merged
  FROM works w JOIN item_links l ON l.item = w.item JOIN items d ON d.id = l.depends_on;

CREATE VIEW waiting_interjections AS
  -- Interjections not yet handed in, in the order they came. Read by: every agent cell; cells 18, 19 and 20.
  SELECT work, id AS interjection, line, speaker, text FROM interjections WHERE delivered_round IS NULL;

CREATE VIEW merge_queue AS
  -- Each project's merge queue: the works at the merge cell, oldest first. Read by: cell 9.
  SELECT i.project, w.id AS work, w.pr, w.pushed_head
  FROM works w JOIN items i ON i.id = w.item
  WHERE w.at = '9' AND w.ended IS NULL
  ORDER BY w.id;

CREATE VIEW side_reports AS
  -- Every side report a round handed in. Read by: the engine, which queues each for the architect; the owner's
  -- interface.
  SELECT r.work, r.n AS round, r.cell, s.value AS report
  FROM rounds r, json_each(r.output, '$.side_reports') s
  WHERE r.output IS NOT NULL;

CREATE VIEW work_followups AS
  -- What to do next, as each report or fix handed in. A return of a flow version before the answer of implementing
  -- and fixing was flat holds it under end. Read by: cell 20; the owner's interface.
  SELECT w.item, r.work, r.n AS round, coalesce(json_extract(r.output, '$.outcome.followup'),
    json_extract(r.output, '$.outcome.end.followup')) AS followup
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE r.cell IN ('4', '8') AND coalesce(json_extract(r.output, '$.outcome.followup'),
    json_extract(r.output, '$.outcome.end.followup')) IS NOT NULL;

CREATE VIEW work_progress AS
  -- What each round said it did, how far a paused or redirected round got included. Read by: the owner's interface.
  SELECT r.work, r.n AS round, r.cell, json_extract(r.output, '$.progress') AS progress, r.ended_at
  FROM rounds r
  WHERE r.output IS NOT NULL;

CREATE VIEW item_research AS
  -- The research and design each work wrote for its item: every accepted own answer of the research, design and
  -- implementation cells. Read by: the owner's interface; the architect.
  SELECT w.item, r.work, r.n AS round, r.cell, json_extract(r.output, '$.outcome') AS outcome
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE r.cell IN ('1', '2', '2a', '3c', '4', '8') AND r.transition IS NOT NULL;

CREATE VIEW adversarial_directions AS
  -- The directions each adversarial round tried. Read by: the owner's interface, to see what kinds of work it
  -- finds problems in.
  SELECT w.item, r.work, r.n AS round, d.value AS direction
  FROM rounds r JOIN works w ON w.id = r.work, json_each(r.output, '$.outcome.directions') d
  WHERE r.cell = '6';

CREATE VIEW advisor_rulings AS
  -- Disputes the fix took to the advisor, and the ruling it carried out. Read by: the owner's interface.
  SELECT r.work, r.n AS round, json_extract(x.value, '$.id') AS finding, json_extract(x.value, '$.ruling') AS ruling
  FROM rounds r, json_each(r.output, '$.outcome.rows') x
  WHERE r.cell = '8' AND json_extract(x.value, '$.answer') = 'advisor_ruled';

CREATE VIEW not_applicable_answers AS
  -- Gate questions answered not applicable, with why. Read by: the owner's interface, to see which questions do
  -- not fit the work.
  SELECT r.work, r.n AS round, r.cell, json_extract(r.output, '$.outcome.answer.why') AS why
  FROM rounds r
  WHERE json_extract(r.output, '$.outcome.answer.verdict') = 'not_applicable';

CREATE VIEW known_red_counts AS
  -- How often main was red only on known red checks, by project. Read by: the owner's interface.
  SELECT i.project, count(*) AS times
  FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
  WHERE r.cell = '10' AND json_extract(r.output, '$.main.kind') = 'known_red'
  GROUP BY i.project;

CREATE VIEW architect_queue AS
  -- The architect's queue: the matters due, not handled and not held, each with who waits for it (an agent: an
  -- engineer stopped at the architect's cell for its question; the owner, for what the owner said or approved; or no
  -- one) and how long it has waited. Read by: cells A0, A4 and A6; the owner's interface.
  SELECT m.id AS matter, m.kind, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w WHERE w.id = m.work AND w.graph = 'engineer' AND w.at = '15' AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind IN ('owner_words', 'owner_approved') THEN 'owner'
      ELSE 'none' END AS waits,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - coalesce(m.due, m.at) AS waited_ms
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.held IS NULL
    AND (m.due IS NULL OR m.due <= CAST(unixepoch('subsec') * 1000 AS INTEGER))
    AND (m.due_item IS NULL OR EXISTS (
      SELECT 1 FROM works w
      WHERE w.id = (SELECT max(x.id) FROM works x WHERE x.item = m.due_item) AND (w.ended IS NOT NULL OR w.at = '20')))
  ORDER BY m.id;

CREATE VIEW architect_state AS
  -- The state of things the architect wrote at its latest first question, with what its wrap-ups learned since.
  -- Read by: cell A1, which corrects it at the next wake; the owner's interface.
  SELECT r.work, r.n AS round, json_extract(r.output, '$.outcome.state') AS state,
    (SELECT json_group_array(json_extract(x.output, '$.outcome.learned')) FROM rounds x JOIN works y ON y.id = x.work
     WHERE y.graph = 'architect' AND x.cell = 'A10' AND x.transition IS NOT NULL
       AND (x.work > r.work OR (x.work = r.work AND x.n > r.n))) AS learned
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE w.graph = 'architect' AND r.cell = 'A1' AND r.transition IS NOT NULL
  ORDER BY r.work DESC, r.n DESC LIMIT 1;

CREATE VIEW architect_log AS
  -- The architect's log, newest first, its latest fifty entries: each judgment it handed in (whether what runs fits,
  -- the crux of a matter, what to do about it and why, the second look), with the round that gave it.
  -- Read by: cells A1, A2 and A8; the owner's interface.
  SELECT r.work, r.n AS round, r.cell, r.ended_at AS at, json_extract(r.output, '$.outcome') AS entry
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE w.graph = 'architect' AND r.cell IN ('A5', 'A7', 'A8', 'A9') AND r.transition IS NOT NULL
  ORDER BY r.work DESC, r.n DESC LIMIT 50;

CREATE VIEW architect_last_fit AS
  -- The architect's latest answer to whether what runs and what starts next fit. Read by: cell A5.
  SELECT r.work, r.n AS round, json_extract(r.output, '$.outcome.fit') AS fit
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE w.graph = 'architect' AND r.cell = 'A5' AND r.transition IS NOT NULL
  ORDER BY r.work DESC, r.n DESC LIMIT 1;

CREATE VIEW wake_handled AS
  -- The matters each wake of the architect handled, in order: the matter picked and the judgment on it, by the round
  -- of the record that closed it. Read by: cells A6 and A10; the owner's interface.
  SELECT r.work, r.n AS round,
    (SELECT json_extract(p.output, '$.outcome.pick.chosen') FROM rounds p
     WHERE p.work = r.work AND p.cell = 'A6' AND p.transition = 'picked' AND p.n < r.n
     ORDER BY p.n DESC LIMIT 1) AS matter,
    (SELECT json_extract(j.output, '$.outcome.judgment') FROM rounds j
     WHERE j.work = r.work AND j.cell = 'A8' AND j.transition IS NOT NULL AND j.n < r.n
     ORDER BY j.n DESC LIMIT 1) AS judgment
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE w.graph = 'architect' AND r.cell = 'A11' AND r.transition IS NOT NULL;

CREATE VIEW wake_changes AS
  -- The changes each wake of the architect collected for the owner's approval, with the matter each belongs to: the
  -- changes of the judgment each record of a matter closed, when it holds any. Read by: cell A15; the owner's
  -- interface.
  SELECT work, round, matter, changes FROM (
    SELECT h.work, h.round, h.matter,
      (SELECT json_extract(j.output, '$.outcome.changes') FROM rounds j
       WHERE j.work = h.work AND j.cell = 'A8' AND j.transition IS NOT NULL AND j.n < h.round
       ORDER BY j.n DESC LIMIT 1) AS changes
    FROM wake_handled h)
  WHERE json_array_length(changes) > 0;

CREATE VIEW policy_current AS
  -- The policy in force: the text written or corrected last before the owner's latest approval, with the round of
  -- that approval; none before the first policy takes effect. Older versions stay readable in the rounds and are
  -- given to no agent. Read by: the architect's cells A1, A3, A5, A6, A8, A9 and P1; the owner's interface.
  SELECT a.work, a.n AS round,
    (SELECT coalesce(json_extract(t.output, '$.outcome.policy'), json_extract(t.output, '$.outcome.revision.policy'))
     FROM rounds t WHERE t.work = a.work AND t.cell IN ('P3', 'P7') AND t.transition IS NOT NULL AND t.n < a.n
     ORDER BY t.n DESC LIMIT 1) AS policy
  FROM rounds a JOIN works w ON w.id = a.work
  WHERE w.graph = 'architect' AND a.cell = 'P10' AND a.transition IS NOT NULL
  ORDER BY a.work DESC, a.n DESC LIMIT 1;

CREATE VIEW policy_draft AS
  -- Each wake's version of the policy being set: the text and the probes, with the answers the architect holds right,
  -- of the latest writing or correcting of the text in the wake. Read by: cells P6 and P7.
  SELECT t.work, t.n AS round,
    coalesce(json_extract(t.output, '$.outcome.policy'), json_extract(t.output, '$.outcome.revision.policy')) AS policy,
    coalesce(json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM rounds t JOIN works w ON w.id = t.work
  WHERE w.graph = 'architect' AND t.cell IN ('P3', 'P7') AND t.transition IS NOT NULL
    AND t.n = (SELECT max(x.n) FROM rounds x
               WHERE x.work = t.work AND x.cell IN ('P3', 'P7') AND x.transition IS NOT NULL);

CREATE VIEW policy_text AS
  -- The text of each wake's version of the policy being set, without its probes. Read by: cells P4 and P5.
  SELECT work, round, policy FROM policy_draft;

CREATE VIEW policy_probes_asked AS
  -- The probes of each wake's version of the policy being set, the questions only, without the answers the architect
  -- holds right. Read by: cell P5, the new architect that reads the policy.
  SELECT d.work, d.round,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.work, d.round;

CREATE VIEW policy_trials AS
  -- Each wake's version of the policy being set, with every reading of it by a new architect since it was written and
  -- the architect's judgment of each, oldest first. Read by: cells P8, P9 and P10; the owner's interface.
  SELECT d.work, d.round, d.policy, d.probes,
    (SELECT json_group_array(json_object('round', r.n, 'reading', json(json_extract(r.output, '$.outcome.reading')),
       'verdict', json((SELECT json_extract(v.output, '$.outcome.verdict') FROM rounds v
                        WHERE v.work = r.work AND v.cell = 'P6' AND v.transition IS NOT NULL AND v.n > r.n
                        ORDER BY v.n LIMIT 1))))
     FROM rounds r WHERE r.work = d.work AND r.cell = 'P5' AND r.transition IS NOT NULL AND r.n > d.round) AS readings
  FROM policy_draft d;

CREATE VIEW plan AS
  -- The plan, one row per segment, in the order works start from them: the segment, and the items that hang on it with
  -- the cell each one's latest work is at. Its gates and budgets are the tables plan_gates and plan_budgets.
  -- Read by: the architect's cells A1, A2, A5, A8 and P1; the owner's interface.
  SELECT s.key AS segment, s.project, s.title, s.stage, s.delivers, s.done_when, s.after, s.assumptions, s.indicators,
    s.size, s.deferred, s.position,
    (SELECT json_group_array(json_object('item', i.id, 'title', i.title, 'cancelled', i.cancelled, 'held', i.held,
       'at', (SELECT w.at FROM works w WHERE w.item = i.id ORDER BY w.id DESC LIMIT 1)))
     FROM items i WHERE i.segment = s.key) AS items
  FROM plan_segments s
  ORDER BY s.deferred IS NOT NULL, s.position IS NULL, s.position, s.key;
