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
-- - Cell, line and transition names are the ones in the flow version of the work they belong to. Cells had numbers and
-- codes first, then names, then names with their stage, and now codes by stage, and the works and rounds of each
-- version keep the ids it gave: a view that reads works or rounds names a cell by its code and every id it has had
-- that is no cell's code now, as ('M2', '9', 'merge_queue', 'merge.queue').
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
  -- Read by: the engine; cells `D1` and `D3`, through the project's design goals.
  project TEXT NOT NULL,
  -- What to do, as an action. Read by: cell `R1` for the items around this one; people.
  title TEXT NOT NULL,
  -- What cannot be done or is wrong, and why the work is worth doing, as written at creation. The engineer's own
  -- understanding replaces them in views once cell `R1` has run; these stay as written.
  -- Read by: cell `R1`, for this item and the items that depend on it; cell `T3` reads the
  -- problem only.
  problem TEXT NOT NULL,
  why TEXT NOT NULL,
  -- How to do it, when the creator already knows; NULL otherwise. Read by: cell `R1`.
  approach TEXT,
  -- What the work hands in: a change to merge, or a report. Read by: cells `R1` and `T4`;
  -- the engine, which runs reports without full CI on the draft.
  deliver TEXT NOT NULL CHECK (deliver IN ('change', 'report')),
  -- Agent level; decides which agent and model run the work. Read by: the engine at cell `S0`.
  level TEXT NOT NULL,
  -- The locked acceptance: a JSON list of acceptance rows in the flow's acceptance shape. NULL until a first design
  -- passes its checks. Every later design must hand in exactly this list; only the advisor can change it.
  -- Read by: cells `D2`, `I1`, `T3`, `T4` and `T1`; the
  -- engine, which compares each design's acceptance with it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round whose acceptance was locked, by its work and its number in the work: a design round, or an advisor
  -- round that changed it. Read by: cell `D2`, to see whether the advisor changed it.
  acceptance_work INTEGER,
  acceptance_round INTEGER,
  -- Why the item was cancelled; NULL while it is not. Read by: the engine; cell `R1` for the items around
  -- this one.
  cancelled TEXT,
  -- For an item imported from outside this flow, such as an item of an earlier board: where it came from, as the import
  -- wrote it; NULL for an item made here. Read by: cell `R1`, through what already exists for the item;
  -- the owner's interface.
  imported_from TEXT,
  -- The branch and the pull request that work on the item had outside this flow, as the import recorded them; NULL when
  -- there were none. Read by: cell `S0`, which takes them over when the item had no work here before; cell
  -- `R1`; the owner's interface.
  imported_branch TEXT,
  imported_pr INTEGER,
  -- The acceptance as the creator wrote it, JSON list of conditions; NULL when the creator wrote none. The locked
  -- acceptance, designed by the work, is acceptance. Read by: cell `R1`, through item_body.
  wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted)),
  -- Why the architect holds the item, with the owner's approval: no work starts on it while it is held; NULL while it
  -- is not. Read by: ready_items; the architect.
  held TEXT,
  -- Its place among its project's items in the architect's order, which works start in, lower first: the architect
  -- sets it with its approved changes, moving an item first, last, or before or after another item of the project,
  -- which ranks the project's items afresh. NULL for an item placed nowhere yet, as one just made is, which comes after
  -- those placed, in the order made. Read by: the dispatch order in ready_items; the architect.
  rank INTEGER,
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE item_links (
  -- Dependencies between items: item cannot finish until depends_on is done. Together they form a directed acyclic
  -- graph, and the engine refuses a write that would close a cycle. Splitting an item means creating new items and
  -- making it depend on them. A blocking dependency keeps item from starting until depends_on is merged; with a
  -- parallel one, item can start while depends_on is unfinished, and waits before it merges. A work already running
  -- goes on either way.
  -- Read by: cell `R1` (the items above and beside this one, what it waits for, who waits for it),
  -- `D8` (what it has been split into), `A4` and `A6` (what the items it waits for handed
  -- in), `M1` (how far they have got); item_blocked_by.
  item INTEGER NOT NULL REFERENCES items(id),
  depends_on INTEGER NOT NULL REFERENCES items(id),
  -- What item needs depends_on to hand in, in one sentence. Read by: cells `R1` and `A6`.
  needs TEXT NOT NULL,
  -- blocking: item does not start until depends_on is merged, as when it needs a module or a library depends_on makes.
  -- parallel: item may start before, and waits where it needs depends_on: before it merges, or where the architect's
  -- reply has it stop. Read by: item_blocked_by; cells `R1`, `D8`, `A4`,
  -- `A6` and `M1`; the architect; the owner's interface.
  kind TEXT NOT NULL CHECK (kind IN ('blocking', 'parallel')),
  PRIMARY KEY (item, depends_on),
  CHECK (item <> depends_on)
) STRICT;

CREATE TABLE dispatch_queue (
  -- Items waiting for a work to start, in the order the owner sets by hand, ahead of the architect's order. When a
  -- slot is free the engine starts a work on the first one. This table goes away once the architect's order alone
  -- decides what starts.
  -- Read by: the engine at cell `S0`.
  item INTEGER PRIMARY KEY REFERENCES items(id),
  -- Lower starts first. A real number, so an item can be put between two others without renumbering.
  position REAL NOT NULL UNIQUE
) STRICT;

CREATE TABLE design_cards (
  -- A change to the design tree for the owner to confirm before it is merged: each decision the change adds, changes
  -- or removes. It asks the owner nothing; what the owner is asked is an ask of the owner. A design card is handed in
  -- by one round and belongs to that round; its item is found through the round's work. A ruled design card never
  -- changes; a decision that has to change comes in a new design card that replaces the old one.
  -- Read by: cell `R1` (design cards ruled on this item), `D1` and `D2` (rulings
  -- that disagreed or chose another option), `D2` (the latest design card of the items that depend on this
  -- one), `T4` and `T5` (the work's design card as it stands), `A2` (the design
  -- card to look at), `M1` (whether the design card has been confirmed).
  id INTEGER PRIMARY KEY,
  -- The round that handed the design card in: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The design card as the round handed it in, JSON, in the flow's design card shape. Copied from that round's
  -- output, which never changes, so the copy cannot drift.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- open: waiting for a ruling. ruled: everyone it waits for has ruled. replaced: a newer design card took its place
  -- and it can no longer be ruled. Read by: the engine; cell `M1`.
  status TEXT NOT NULL CHECK (status IN ('open', 'ruled', 'replaced')),
  -- The rulings in the order given, JSON list, the owner's and the architect's alike: who ruled and when, and a verdict
  -- on each decision of the card by its number (confirm or disagree), with any approach chosen instead and why; a
  -- ruling made before rulings were per decision holds one verdict on the whole card, its option and words. The card is
  -- confirmed when a ruling confirms every decision. Read by: cells `R1`, `D1`,
  -- `D2` and `M1`.
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
  -- Read by: cell `R1` (the asks of the owner on this item, with their answers); the engine, which waits
  -- for the answer.
  id INTEGER PRIMARY KEY,
  -- The round that put it to the owner: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- What is asked, JSON: whom it asks, the work and item, and what the asking cell handed in. Copied from what that
  -- round was given, which never changes.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The owner's answer, JSON: who answered and when, and the text; for an ask of the architect, the rest of the
  -- architect's reply too. NULL while it waits. Read by: the engine; cell `R1`.
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
  -- a design card posted, every work halted until a usage limit resets, a fault of wf-agents, and the like. It needs no
  -- answer. The same text goes to the notifications log.
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
  ask INTEGER REFERENCES owner_asks(id),
  -- When the notice reports a fault of wf-agents, its kind: cache_miss, rounds that missed their session's prompt
  -- cache, each kept in cache_misses; flow_anomaly, an ask of the owner that came of a row counting a flow anomaly,
  -- where the way on an engineer chose did not fit the situation. wf-agents runs other projects too, and such a fault
  -- is its own, to be fixed in wf-agents whichever project it came up in. NULL for another notice. Read by: the
  -- owner's interface, which sets such a notice apart.
  fault TEXT
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
  -- One run of a graph of the flow: an engineer's work on an item, or one wake of a project's architect. It holds the
  -- current
  -- state the engine needs to take the next step, written in the same transaction as the round that changed it; the
  -- history is in rounds. Which cell brought the work to its cell, whether it is done, what it waits for and when it
  -- started and ended are derived from its rounds. At most one work per item runs at a time.
  -- Read by: the engine at every step.
  id INTEGER PRIMARY KEY,
  -- The item worked on; NULL for an architect wake, which belongs to no item but to its project.
  item INTEGER REFERENCES items(id),
  flow_version INTEGER NOT NULL REFERENCES flow_versions(id),
  -- Which graph of that version it runs. Read by: the engine.
  graph TEXT NOT NULL CHECK (graph IN ('engineer', 'architect')),
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
  -- For a wake of the architect, the project whose architect it is; NULL for an engineer's work, whose project is its
  -- item's. Read by: the engine, which runs one wake of each project's architect at a time and reads the project's
  -- views for it; the architect's views; the owner's interface.
  project TEXT CHECK (project IS NULL OR graph = 'architect')
) STRICT;

CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session's context is past the configuration's limit, when the model its rounds run on
  -- changed, whose prompt cache it would have to write again whole, and when the owner asks for one. Each project's
  -- architect's line goes on in one CLI session across the project's wakes: each wake has a row of its own with the
  -- same cli_session, until a new policy ends it. A cell on no line has a session of its own for each round, ended
  -- once the round has.
  -- Hidden from agents.
  -- Read by: the session manager, to resume a line, to start a new session and to keep a waiting session warm; the
  -- owner's interface.
  id INTEGER PRIMARY KEY,
  work INTEGER NOT NULL REFERENCES works(id),
  -- The line, for example engineer, review, adversarial, advisor or architect; for a cell on no line, the cell.
  line TEXT NOT NULL,
  -- The CLI that started the session. Only that CLI can resume it, so this stays when the configuration changes.
  cli TEXT NOT NULL,
  -- The CLI's own id for the session, passed to it to resume.
  cli_session TEXT NOT NULL,
  -- When the session was ended, so that the line's next round starts a new one; NULL while it goes on. A work's line
  -- has at most one session going on. Read by: the engine, which resumes only a session not ended.
  ended INTEGER,
  -- Why it started: line, the line had no session going on (its first round in the work, or after its session
  -- ended); cell, the round is the work's first at a cell the line starts a new session at; limit, the context of the
  -- line's session before it was past the configuration's limit; model, the line's rounds run on another model than its
  -- session before; wake, a wake of a project's architect going on in the CLI session the project's last wake left;
  -- owner, the owner asked for a new session with the round's Start or Step on the page. Read by: the owner's
  -- interface.
  started TEXT NOT NULL DEFAULT 'line' CHECK (started IN ('line', 'cell', 'limit', 'model', 'wake', 'owner'))
) STRICT;

CREATE TABLE pings (
  -- Each keep-warm ping: one minimal message the session manager sent a session whose work waited, 55 minutes after
  -- the session's last call, so that the session's prompt cache was read before it expired instead of written again
  -- whole at its next round. A ping means the work waited over an hour, which is an anomaly: it says where the work
  -- waited and for what. Its cost is the work's.
  -- Hidden from agents.
  -- Read by: the session manager, which counts a wait's pings and a session's cost; the owner's interface and the
  -- anomalies command.
  id INTEGER PRIMARY KEY,
  session INTEGER NOT NULL REFERENCES sessions(id),
  -- Where the work waited and for what, JSON: the cell, what it waited on (pause, owner, architect, items or tool)
  -- and that in words.
  waited TEXT NOT NULL CHECK (json_valid(waited)),
  -- The model the ping ran on, as the CLI reported it.
  model TEXT,
  -- Tokens and cost as the CLI reported them, JSON, with the ping's own cost as a round's usage has it; NULL while it
  -- runs.
  usage TEXT CHECK (usage IS NULL OR json_valid(usage)),
  -- How many times the CLI compacted the session during the ping: the session then holds nothing from before.
  compactions INTEGER NOT NULL DEFAULT 0 CHECK (compactions >= 0),
  -- Why the run failed, when it did.
  error TEXT,
  -- When it started and ended.
  started_at INTEGER NOT NULL,
  ended_at INTEGER
) STRICT;

CREATE TABLE cache_misses (
  -- Each round that missed its session's prompt cache: it resumed the session within the cache's lifetime since the
  -- session's last call, a round's or a keep-warm ping's, and still wrote to the cache again at least half of what the
  -- session held, the last prompt of its round before as recorded. A miss is a fault of wf-agents, whichever project
  -- it came up in: the engine tells the owner in a notice of the fault, and a further miss with the same cause on the
  -- same line of the same project, while that notice is unread, adds to it instead of making a new one. Written once,
  -- never changed.
  -- Hidden from agents.
  -- Read by: the engine, which finds the notice a miss adds to; the owner's interface, which shows with each notice
  -- how many misses it holds, the tokens they wrote again and what they cost.
  -- The round that missed: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The line of its session.
  line TEXT NOT NULL,
  -- The cause as far as the record tells it, JSON: each part of the command line that differs from the session's
  -- round before, with its value there and in this round; an empty list when none differs.
  differs TEXT NOT NULL CHECK (json_valid(differs)),
  -- The tokens the round wrote to the cache again: those its first call wrote, or the whole round's when its record
  -- does not list its calls.
  written INTEGER NOT NULL CHECK (written >= 0),
  -- The tokens the session held: the last prompt of its round before, as recorded.
  held INTEGER NOT NULL CHECK (held > 0),
  -- What writing them again cost beyond reading them, in US dollars, priced from what the round cost; NULL when its
  -- record does not tell.
  cost_usd REAL,
  -- The notice that tells the owner of it.
  notice INTEGER NOT NULL REFERENCES engine_notices(id),
  PRIMARY KEY (work, round),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE owner_switches (
  -- The owner's switches on the page that hold across restarts of the engine, one row for each switch the owner set;
  -- a switch with no row is as it is at first. Who set one, and when, is in changes.
  -- Hidden from agents.
  -- Read by: the engine; the owner's interface.
  -- keep_warm_paused: whether the session manager keeps the sessions of waiting works warm while the owner's global
  -- pause holds; on at first.
  name TEXT PRIMARY KEY CHECK (name IN ('keep_warm_paused')),
  -- Whether it is on.
  value INTEGER NOT NULL CHECK (value IN (0, 1))
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
  -- the values of its inputs; for cell `start`, nothing. Rounds recorded before this format keep the prompt and inputs.
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
  -- Each attempt of a tool the engine runs: the tools an agent asks for, run at cell `X1`, and the tool cells
  -- `T2`, `M2` and `L1`. A retry is a new row.
  -- Read by: the round that asked for the tools (their results); cells `T3` and `T1` (each
  -- measurement); `T4` and `T5` (the latest CI on the work); `A2` (a tool that
  -- still failed after retries); the history.
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
  -- Read by: every agent cell; cells `T7`, `M1` and `E1` when the work stands at them.
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
  -- For the notice that an item this work's item depended on was cancelled: that item; NULL for anything else said.
  -- The cells where the work waits (`A2`, `A6`, `M1`) take such a notice to choose the way on by a row of their own,
  -- as an item they waited for that will never deliver. Read by: the engine's value `cancelled`.
  cancelled_item INTEGER REFERENCES items(id),
  FOREIGN KEY (work, delivered_round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE known_red_checks (
  -- CI checks known to fail for a reason outside any one work, so full CI and the merge queue do not hold them against
  -- the work. Added while works run, which is why this is a table and not configuration. A row is removed when its
  -- check is fixed. How often a check failed now and then is counted from tool_runs.
  -- Read by: cells `T2` and `M2`.
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
  -- A matter for a project's architect: an engineer's question, a report, a design card to look at, a failure, the
  -- owner's words, a message from another project's architect, slots free with nothing of the project ready, or a
  -- place its understanding found to set straight. The project's queue is its matters not handled; its architect
  -- picks among them by the constitution and the policy, not by the order they came. A matter in the queue wakes the
  -- project's architect.
  -- Read by: the engine, which wakes the architect, picks a lone matter, routes each to its path, hands it its material
  -- and takes its replies back; the architect's cells through architect_queue and wake_handled; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- The project whose architect it is for.
  project TEXT NOT NULL,
  -- Its kind, which decides the path it takes: set_straight, engineer_question, report, design_card, failure,
  -- owner_words, other_architect or slots_free. A matter handled before these kinds keeps the kind it had then.
  kind TEXT NOT NULL,
  -- What more its kind says: an engineer's question's own kind (split, duplicate, ...); a report's (a side report's
  -- kind, or followup for a report's what to do next); a failure's (tool_failed, environment_changed); the owner's
  -- words' (said, answer, release, approved_changes); a message's (message, reply); the kind of a place to set
  -- straight. NULL when there is nothing more. Read by: the engine, for the matter's material; the owner's interface.
  detail TEXT,
  -- What it is, in one sentence, as the engine wrote it.
  what TEXT NOT NULL,
  -- Its material, JSON: what the asking cell handed in, the side report, a report's what to do next with its paths and
  -- the commit merged, the design card, the tool that failed, the owner's words, the message, the place to set
  -- straight. Read by: the engine, for the matter's material.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The item it concerns; NULL when it concerns none.
  item INTEGER REFERENCES items(id),
  -- The round that raised it: an engineer's round at the architect's cell, a round that handed in a side report or a
  -- design card, an architect's round; NULL for one the owner or the engine raised.
  work INTEGER,
  round INTEGER,
  -- When it came. Read by: the queue, for how long it waited.
  at INTEGER NOT NULL,
  -- The ask of the owner a cell on its path put about it, which holds it while the owner has not answered: it is then
  -- out of the queue, and once answered it is back, and goes back to the cell that asked, with the answer. NULL when no
  -- cell asked about it. Read by: the queue; the engine, which routes it and gives the answer.
  asked INTEGER REFERENCES owner_asks(id),
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer.
  resume_cell TEXT,
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. Read by: the engine, which
  -- takes it back unchanged to the work that asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the item changes of its round to be made, JSON, as the round handed it in: it becomes the
  -- reply, with the numbers of the items they made, once the review of their batch lets them be made. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its wake and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; NULL otherwise. Read by: architect_queue, which leaves it out; the engine, which tells a new
  -- work on the item what an earlier work asked and had withdrawn; the owner's interface, which leaves it out of the
  -- queue.
  withdrawn TEXT,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE architect_item_changes (
  -- A change to the items a project's architect handed in while handling a matter: collected into its batch, made only
  -- once the review of the whole batch finds it sound, and kept as the log of every change made or dropped.
  -- Read by: the engine, which gives the batch to the review and makes the changes; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- The round that handed it in: a wake's round on a matter's path, or a revision of the batch.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The matter it came from.
  matter INTEGER NOT NULL REFERENCES architect_matters(id),
  -- The change, JSON, in the flow's item_change shape, with its reason.
  change TEXT NOT NULL CHECK (json_valid(change)),
  -- collected: in the batch, waiting for the review; made: the review found it sound and the engine made it; revised: a
  -- revision of the batch took its place; dropped: the review still found it wrong at its third look, and the owner
  -- was asked.
  state TEXT NOT NULL CHECK (state IN ('collected', 'made', 'revised', 'dropped')),
  -- For a change made, what it made, JSON, such as the number of the item it created; for one dropped, what the review
  -- found. NULL otherwise. Read by: the owner's interface.
  outcome TEXT CHECK (outcome IS NULL OR json_valid(outcome)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE observations (
  -- Something worth doing in a project that no item does yet, as its architect recorded it from a report or its own
  -- finding: a record beside the items. The same thing reported again is counted, not recorded twice; an item takes
  -- observations in, and they close once its work merges. Who recorded or changed one, and when, is in changes.
  -- Read by: the architect's cells, through observations_open and the engine's overview; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- The project it belongs to.
  project TEXT NOT NULL,
  -- What it is, why it is worth doing, and what happens if it is not done.
  what TEXT NOT NULL,
  -- Its tags, JSON list, each one of the project's set in its configuration.
  tags TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags)),
  -- Who found it, in what situation, and the matter it came from.
  source TEXT NOT NULL,
  -- How many times it was reported: 1 when recorded, one more for each report merged into it.
  times_reported INTEGER NOT NULL DEFAULT 1 CHECK (times_reported >= 1),
  -- What it refers to, JSON list: PRs, branches, design cards, the owner's words, and what later reports added.
  refs TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(refs)),
  -- The item that took it in, which closes it once its work merges; NULL while no item has.
  folded_into INTEGER REFERENCES items(id),
  -- Why it was closed: outdated, folded (an item took it in), owner (the owner closed it on the page) or other; NULL
  -- while it is open, and for one an item took in, which closes once that item's work merges.
  closed TEXT CHECK (closed IN ('outdated', 'folded', 'owner', 'other')),
  -- What made it so, in a sentence; set with closed.
  closed_why TEXT,
  CHECK ((closed IS NULL) = (closed_why IS NULL))
) STRICT;

CREATE TABLE dispatch_holds (
  -- What stops new works of a project from starting, one row for each: its architect's own hold, its architect setting
  -- it straight, its architect setting a new policy. While a project has a row here, the engine starts no new work on
  -- its items; works already running go on, and the other projects' works start as usual.
  -- Read by: the engine at cell `S0`; the architect, through its board; the owner's interface.
  project TEXT NOT NULL,
  -- architect: held by the architect's own action. set_straight: its understanding found the whole must be set
  -- straight. policy: a new policy is being set.
  reason TEXT NOT NULL CHECK (reason IN ('architect', 'set_straight', 'policy')),
  -- When it began.
  since INTEGER NOT NULL,
  -- Why, in the owner's language, as the architect said it for its own hold; NULL for the others, whose reason says
  -- why. Read by: the owner's interface.
  why TEXT,
  PRIMARY KEY (project, reason)
) STRICT;

-- Indexes for the reads every step makes.

-- What an item depends on is the primary key; who depends on an item needs this.
CREATE INDEX item_links_by_depends_on ON item_links (depends_on);
-- The works of an item.
CREATE INDEX works_by_item ON works (item);
-- The session each line of a work goes on in: one at most, among those not ended.
CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;
-- The keep-warm pings of a session.
CREATE INDEX pings_by_session ON pings (session);
-- The cache misses a notice tells of.
CREATE INDEX cache_misses_by_notice ON cache_misses (notice);
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
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
-- The item changes of a batch still waiting for its review.
CREATE INDEX architect_item_changes_collected ON architect_item_changes (work) WHERE state = 'collected';
-- A project's observations.
CREATE INDEX observations_by_project ON observations (project);

-- Views: what cells read and the owner's interface shows, computed from the tables. A view a cell reads for the item,
-- work or project at hand has a column of that name; the engine filters on it.

CREATE VIEW item_blocked_by AS
  -- Each item with each item it depends on by a blocking dependency that is not merged yet, as the merge cell
  -- (`M2`) records a merge: no work starts on the item until every one is. A cancelled item, which never
  -- delivers, holds back none. Read by: ready_items; the architect, through its board; the owner's interface, which
  -- shows an item that would start but for them as waiting for them.
  SELECT l.item, l.depends_on, l.needs
  FROM item_links l JOIN items d ON d.id = l.depends_on
  WHERE l.kind = 'blocking' AND d.cancelled IS NULL AND NOT EXISTS (
    SELECT 1 FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = l.depends_on AND r.cell IN ('M2', '9', 'merge_queue', 'merge.queue') AND r.transition = 'merged');

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: first those the owner queued, in the owner's order; then the
  -- others in the architect's order, each by its rank among its project's items, lower first, those placed nowhere yet
  -- after, in the order made. An item is ready when it is not cancelled, not held, has no work running and no item it
  -- depends on by a blocking dependency unmerged (item_blocked_by), and, unless the owner queued it, is not done. An
  -- item leaves the owner's queue when a work starts on it, and may be queued again once its works ended. A work runs
  -- until it ends or its last round, at the cell it is at, completed without taking a row, which only the end of its
  -- graph does; it is done at the end of the engineer's graph. While dispatch_holds has a row, the engine starts none.
  -- Read by: cell `S0`; the architect, through what runs and starts next.
  WITH
  running(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.graph = 'engineer' AND w.at IN ('E1', '20', 'end'))
  SELECT i.id AS item, i.project, i.title, i.level, q.position, i.rank
  FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND i.held IS NULL AND i.id NOT IN (SELECT item FROM running)
    AND i.id NOT IN (SELECT item FROM item_blocked_by)
    AND (q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

CREATE VIEW item_body AS
  -- The item as its creator wrote it, the acceptance it wanted included. Read by: cells `R1` and
  -- `T4`.
  SELECT id AS item, project, title, problem, why, approach, deliver, wanted FROM items;

CREATE VIEW item_problem AS
  -- The item's problem: the engineer's understanding once cell `R1` has run, as written before.
  -- Read by: cell `T3`, which is given the problem and the acceptance only.
  SELECT i.id AS item, coalesce((
    SELECT json_extract(r.output, '$.outcome.understanding.problem')
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = i.id AND r.cell IN ('R1', '1', 'research', 'research.problem') AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.understanding') IS NOT NULL
    ORDER BY r.work DESC, r.n DESC LIMIT 1), i.problem) AS problem
  FROM items i;

CREATE VIEW item_acceptance AS
  -- The item's locked acceptance, and the round that locked it, by its work and its number in the work; NULL until a
  -- first design passes its checks. Read by: cells `D2`, `I1`, `T3`,
  -- `T4` and `T1`.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;

CREATE VIEW item_design_cards AS
  -- Every design card of the item's works, with its rulings. Read by: cells `R1`, `D1`,
  -- `D2` and `M1`.
  SELECT w.item, c.work, c.id AS design_card, c.round, c.body, c.status, c.rulings, c.replaced_by
  FROM design_cards c JOIN works w ON w.id = c.work;

CREATE VIEW item_asks AS
  -- Every ask of the owner of the item's works, with its answer. Read by: cell `R1`.
  SELECT w.item, a.work, a.id AS ask, a.round, a.body, a.answer
  FROM owner_asks a JOIN works w ON w.id = a.work;

CREATE VIEW parent_design_cards AS
  -- The design cards of the items that depend on this one, not yet replaced: what a split-off item's parent
  -- decided. Read by: cell `D2`.
  SELECT l.depends_on AS item, l.item AS parent, c.id AS design_card, c.body, c.status, c.rulings
  FROM item_links l
  JOIN works w ON w.item = l.item
  JOIN design_cards c ON c.work = w.id
  WHERE c.status <> 'replaced';

CREATE VIEW item_dependencies AS
  -- What the item depends on, with the kind of each dependency, what it needs of each and how far each is: the cell
  -- its latest work is at, or why it was cancelled. Read by: cells `D8` and `M1`.
  SELECT l.item, l.depends_on, l.kind, l.needs, d.title, d.cancelled,
    (SELECT w.at FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS at,
    (SELECT w.pr FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS pr
  FROM item_links l JOIN items d ON d.id = l.depends_on;

CREATE VIEW item_context AS
  -- Where the item sits in the whole: the items above it (those that depend on it, up to the root), beside it
  -- (the other items its parents depend on) and below it (those it depends on), each with its problem and why and
  -- the kind of the dependency that brings it in: above, how it depends on the item below it on the way; beside, how
  -- their parent depends on it; below, how this item depends on it. Read by: cell `R1`.
  WITH RECURSIVE up(item, other, depth, kind) AS (
    SELECT depends_on, item, 1, kind FROM item_links
    UNION
    SELECT up.item, l.item, up.depth + 1, l.kind FROM up JOIN item_links l ON l.depends_on = up.other
  )
  SELECT up.item, 'above' AS relation, up.depth, up.kind, o.id AS other, o.title, o.problem, o.why, o.cancelled
  FROM up JOIN items o ON o.id = up.other
  UNION ALL
  SELECT a.depends_on, 'beside', 1, b.kind, o.id, o.title, o.problem, o.why, o.cancelled
  FROM item_links a JOIN item_links b ON b.item = a.item AND b.depends_on <> a.depends_on
  JOIN items o ON o.id = b.depends_on
  UNION ALL
  SELECT l.item, 'below', 1, l.kind, o.id, o.title, o.problem, o.why, o.cancelled
  FROM item_links l JOIN items o ON o.id = l.depends_on;

CREATE VIEW work_state AS
  -- Where each work stands, as the engine keeps it. Read by: cells `T2`, `M2`, `A5`, `X1`,
  -- `T7` and `E1`.
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
  -- Read by: cell `A1`; cell `A4`, through work_asking; the owner's interface.
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
  -- way on needs of where the work stands. Read by: cell `A4`.
  SELECT w.id AS work, w.resume_at AS asked_at,
    coalesce((SELECT c.times FROM work_counters c WHERE c.work = w.id AND c.counter = 'asks'), 0) AS asks
  FROM works w;

CREATE VIEW work_design_card AS
  -- Each work's design card as it stands: its latest one, unless that was replaced, with its rulings and the numbers of
  -- the earlier design cards of the work, which it replaced. Read by: cells `T4` and `T5`.
  SELECT c.work, c.id AS design_card, c.round, c.body, c.status, c.rulings,
    (SELECT json_group_array(o.id) FROM design_cards o WHERE o.work = c.work AND o.id < c.id) AS replaces
  FROM design_cards c
  WHERE c.status <> 'replaced' AND c.id = (SELECT max(o.id) FROM design_cards o WHERE o.work = c.work);

CREATE VIEW work_ci AS
  -- The CI on each work: the latest run of the fast CI (the tool ci), of the full CI on the draft and of the tests on
  -- the code before the change (the tool ci_before), each with the head it ran on and its result, which holds the
  -- links to its runs; and the head last pushed. Read by: cells `T4` and `T5`.
  SELECT w.id AS work, w.pushed_head AS head,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'ci' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS fast_ci,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'full_ci' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS full_ci,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'ci_before' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS ci_before
  FROM works w;

CREATE VIEW work_measurements AS
  -- Every run of the measure tool for each work, with its result and the files kept. Read by: cells `T3`
  -- and `T1`.
  SELECT t.work, t.round, t.attempt, t.args, t.result, t.files
  FROM tool_runs t
  WHERE t.tool = 'measure';

CREATE VIEW work_review_record AS
  -- What decides what runs next after a review: the values rows recorded, and the last return of the table review
  -- and of adversarial testing. Read by: cell `T7`.
  SELECT w.id AS work, w.recorded,
    (SELECT r.output FROM rounds r WHERE r.work = w.id AND r.cell IN ('T4', '5a', 'table_review', 'test.table_review')
      AND r.transition IS NOT NULL
     ORDER BY r.n DESC LIMIT 1) AS last_review,
    (SELECT r.output FROM rounds r WHERE r.work = w.id AND r.cell IN ('T1', '6', 'adversarial', 'test.adversarial')
      AND r.transition IS NOT NULL
     ORDER BY r.n DESC LIMIT 1) AS last_adversarial
  FROM works w;

CREATE VIEW waited_items AS
  -- The items each work's item depends on, with the kind of each dependency, what it needs of each, how far each is
  -- and, once merged, the last return of its merge. Read by: cells `A4` and `A6`.
  SELECT w.id AS work, d.id AS item, l.kind, l.needs, d.title, d.cancelled,
    (SELECT x.at FROM works x WHERE x.item = d.id ORDER BY x.id DESC LIMIT 1) AS at,
    (SELECT r.output FROM rounds r JOIN works x ON x.id = r.work
     WHERE x.item = d.id AND r.cell IN ('M2', '9', 'merge_queue', 'merge.queue') AND r.transition = 'merged'
     ORDER BY r.work DESC, r.n DESC LIMIT 1) AS merged
  FROM works w JOIN item_links l ON l.item = w.item JOIN items d ON d.id = l.depends_on;

CREATE VIEW waiting_interjections AS
  -- Interjections not yet handed in, in the order they came. Read by: every agent cell; cells `T7`,
  -- `M1` and `E1`.
  SELECT work, id AS interjection, line, speaker, text FROM interjections WHERE delivered_round IS NULL;

CREATE VIEW merge_queue AS
  -- Each project's merge queue: the works at the merge cell, oldest first. Read by: cell `M2`.
  SELECT i.project, w.id AS work, w.pr, w.pushed_head
  FROM works w JOIN items i ON i.id = w.item
  WHERE w.at IN ('M2', '9', 'merge_queue', 'merge.queue') AND w.ended IS NULL
  ORDER BY w.id;

CREATE VIEW side_reports AS
  -- Every side report a round handed in. Read by: the engine, which queues each for the architect; the owner's
  -- interface.
  SELECT r.work, r.n AS round, r.cell, s.value AS report
  FROM rounds r, json_each(r.output, '$.side_reports') s
  WHERE r.output IS NOT NULL;

CREATE VIEW work_followups AS
  -- What to do next, as each report or fix handed in. A return of a flow version before the answer of implementing
  -- and fixing was flat holds it under end. Read by: cell `E1`, whose step queues a report's latest for the architect;
  -- the owner's interface.
  SELECT w.item, r.work, r.n AS round, coalesce(json_extract(r.output, '$.outcome.followup'),
    json_extract(r.output, '$.outcome.end.followup')) AS followup
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE r.cell IN ('I1', '4', 'implement', 'T6', '8', 'fix', 'test.fix')
    AND coalesce(json_extract(r.output, '$.outcome.followup'),
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
  WHERE r.cell IN ('R1', '1', 'research', 'research.problem', 'D1', '2', 'compare', 'design.compare', 'D2', '2a',
    'choose', 'design.choose', 'D6', '3c', 'cost', 'design.cost', 'I1', '4', 'implement', 'T6', '8', 'fix', 'test.fix')
    AND r.transition IS NOT NULL;

CREATE VIEW adversarial_directions AS
  -- The directions each adversarial round tried. Read by: the owner's interface, to see what kinds of work it
  -- finds problems in.
  SELECT w.item, r.work, r.n AS round, d.value AS direction
  FROM rounds r JOIN works w ON w.id = r.work, json_each(r.output, '$.outcome.directions') d
  WHERE r.cell IN ('T1', '6', 'adversarial', 'test.adversarial');

CREATE VIEW advisor_rulings AS
  -- Disputes the fix took to the advisor, and the ruling it carried out. Read by: the owner's interface.
  SELECT r.work, r.n AS round, json_extract(x.value, '$.id') AS finding, json_extract(x.value, '$.ruling') AS ruling
  FROM rounds r, json_each(r.output, '$.outcome.rows') x
  WHERE r.cell IN ('T6', '8', 'fix', 'test.fix') AND json_extract(x.value, '$.answer') = 'advisor_ruled';

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
  WHERE r.cell IN ('L1', '10', 'watch_main') AND json_extract(r.output, '$.main.kind') = 'known_red'
  GROUP BY i.project;

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell for its question; the owner, for
  -- the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer, and whether it is urgent. Read by: cells `W0` and `Q0`; the engine, which wakes each
  -- project's architect and gives the architect its queue; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AS answered,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;

CREATE VIEW architect_notes AS
  -- Every note each project's architect wrote, oldest first: about its project as a whole, or about one item, in the
  -- effects of a round on a matter's path or at a wrap-up. Read by: architect_state; the owner's interface.
  SELECT w.project, r.work, r.n AS round, r.cell, json_extract(n.value, '$.item') AS item,
    json_extract(n.value, '$.text') AS text
  FROM rounds r JOIN works w ON w.id = r.work,
    json_each(coalesce(json_extract(r.output, '$.outcome.effects.notes'), json_extract(r.output, '$.outcome.notes'))) n
  WHERE w.graph = 'architect' AND r.transition IS NOT NULL
  ORDER BY r.work, r.n;

CREATE VIEW architect_state AS
  -- Each project's state of things as its architect last wrote it: what the project is now, as its latest
  -- understanding wrote it (cell `U1`), with what its wrap-ups learned and every note written since; a round that asked
  -- for tools or paused wrote none of them. Read by: cells `U1` and `E1`; the owner's interface.
  SELECT project, work, round, state, learned, notes FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.state') AS state,
      (SELECT json_group_array(json_extract(x.output, '$.outcome.learned')) FROM rounds x JOIN works y ON y.id = x.work
       WHERE y.graph = 'architect' AND y.project = w.project AND x.cell IN ('E1', 'wrap_up')
         AND x.transition IS NOT NULL
         AND x.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
         AND (x.work > r.work OR (x.work = r.work AND x.n > r.n))) AS learned,
      (SELECT json_group_array(json_object('item', n.item, 'text', n.text)) FROM architect_notes n
       WHERE n.project = w.project AND (n.work > r.work OR (n.work = r.work AND n.round > r.n))) AS notes,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('U1', 'state', 'understand.state') AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner'))
  WHERE k = 1;

CREATE VIEW architect_log AS
  -- Each project's architect's log, newest first, its latest forty entries: each judgment it handed in (whether the
  -- policy is right, whether what runs fits, each matter's answer, each rethink after a batch) and the owner's answer
  -- to each study of a new policy, by the round that gave it, with the matter it handled and what it said, its
  -- conclusion with its reasons. Read by: cells `U1`, `P1` and `N1`.
  SELECT project, work, round, cell, at, matter, said FROM (
    SELECT w.project, r.work, r.n AS round, r.cell, r.ended_at AS at,
      (SELECT m.id FROM architect_matters m WHERE m.handled_work = r.work AND m.handled_round = r.n) AS matter,
      coalesce(json_extract(r.output, '$.progress'),
        json_extract(r.output, '$.ruling.choice') || ': ' || json_extract(r.output, '$.ruling.text')) AS said,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
      AND r.cell IN ('P1', 'U2', 'policy_right', 'understand.policy', 'N1', 'U3', 'fit', 'understand.fit', 'H1', 'T1',
        'set_straight', 'handle.set_straight', 'H3', 'ways', 'handle.ways', 'H5', 'I2', 'take_in', 'handle.take_in',
        'H6', 'K1', 'card_sound', 'handle.card_sound', 'H7', 'F1', 'have_fixed', 'handle.have_fixed', 'H8', 'O1',
        'owner_words', 'handle.owner_words', 'H9', 'M1', 'answer_architect', 'handle.answer_architect', 'H10',
        'what_runs', 'handle.what_runs', 'B4', 'C1', 'rethink', 'batch.rethink', 'P3', 'owner_reads',
        'policy.owner_reads'))
  WHERE k <= 40;

CREATE VIEW architect_last_fit AS
  -- Each project's architect's latest plan of what to do next: what was missing, what mattered most, and whether what
  -- ran fit; a plan of a flow version before what was missing and what mattered most were asked holds only the last.
  -- Read by: cell `N1`.
  SELECT project, work, round, missing, first, fit FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.missing') AS missing,
      json_extract(r.output, '$.outcome.first') AS first, json_extract(r.output, '$.outcome.fit') AS fit,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('N1', 'U3', 'fit', 'understand.fit') AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.fit') IS NOT NULL)
  WHERE k = 1;

CREATE VIEW wake_handled AS
  -- The matters each wake of an architect handled, in the order it handled them: each matter, its kind, the cell and
  -- the round that closed it, and what that round said it did. Read by: cell `E1`; the owner's interface.
  SELECT m.handled_work AS work, m.handled_round AS round, m.id AS matter, m.kind, m.detail, m.what, r.cell,
    json_extract(r.output, '$.progress') AS said
  FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
  ORDER BY m.handled_work, m.handled_round;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written, or put together from a correction, last before the owner's latest
  -- approval in a wake of the project's architect, with the round of that approval; none before the first policy takes
  -- effect. A correction hands in only what it changes, and the engine puts the text together (`P5`); a
  -- correction of a flow version before that held the whole text. Older versions stay readable in the rounds and are
  -- given to no agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
         json_extract(t.output, '$.outcome.revision.policy'))
       FROM rounds t
       WHERE t.work = a.work AND t.cell IN ('P4', 'write', 'policy.write', 'P5', 'assemble', 'policy.assemble', 'P8',
         'correct', 'policy.correct')
         AND t.transition IS NOT NULL AND t.n < a.n
         AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
       ORDER BY t.n DESC LIMIT 1) AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY a.work DESC, a.n DESC) AS k
    FROM rounds a JOIN works w ON w.id = a.work
    WHERE w.graph = 'architect' AND a.cell IN ('P12', 'takes_effect', 'policy.takes_effect')
      AND a.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW policy_draft AS
  -- Each wake's version of the policy being set: the round of the latest writing or correcting in the wake, the text as
  -- it stands, and the probes with the answers the architect holds right. A correction hands in only what it changes,
  -- and the text is then as the engine last put it together (`P5`); a correction of a flow version before
  -- that held the whole text. Read by: policy_text, policy_probes_asked and policy_trials; the cells on the architect's
  -- line take it from the returns that wrote it or put it together.
  SELECT t.work, t.n AS round,
    (SELECT coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
       json_extract(x.output, '$.outcome.revision.policy'))
     FROM rounds x
     WHERE x.work = t.work AND x.cell IN ('P4', 'write', 'policy.write', 'P5', 'assemble', 'policy.assemble', 'P8',
       'correct', 'policy.correct') AND x.transition IS NOT NULL
       AND coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
         json_extract(x.output, '$.outcome.revision.policy')) IS NOT NULL
     ORDER BY x.n DESC LIMIT 1) AS policy,
    coalesce(json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM rounds t JOIN works w ON w.id = t.work
  WHERE w.graph = 'architect' AND t.cell IN ('P4', 'write', 'policy.write', 'P8', 'correct', 'policy.correct')
    AND t.transition IS NOT NULL
    AND t.n = (SELECT max(x.n) FROM rounds x
               WHERE x.work = t.work AND x.cell IN ('P4', 'write', 'policy.write', 'P8', 'correct', 'policy.correct')
                 AND x.transition IS NOT NULL);

CREATE VIEW policy_text AS
  -- The text of each wake's version of the policy being set, without its probes. Read by: cell `P5`; the
  -- engine, to check a reading of it.
  SELECT work, round, policy FROM policy_draft;

CREATE VIEW policy_probes_asked AS
  -- The probes of each wake's version of the policy being set, the questions only, without the answers the architect
  -- holds right. Read by: cell `P6`, the new architect that reads the policy.
  SELECT d.work, d.round,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.work, d.round;

CREATE VIEW policy_trials AS
  -- Each wake's version of the policy being set, with every reading of it by a new architect since it was written and
  -- the architect's judgment of each, oldest first. Read by: cells `P9`, `P11` and
  -- `P12`; the owner's interface.
  SELECT d.work, d.round, d.policy, d.probes,
    (SELECT json_group_array(json_object('round', r.n, 'reading', json(json_extract(r.output, '$.outcome.reading')),
       'verdict', json((SELECT json_extract(v.output, '$.outcome.verdict') FROM rounds v
                        WHERE v.work = r.work AND v.cell IN ('P7', 'judge_read', 'policy.judge_read')
                          AND v.transition IS NOT NULL
                          AND v.n > r.n
                        ORDER BY v.n LIMIT 1))))
     FROM rounds r
     WHERE r.work = d.work AND r.cell IN ('P6', 'fresh_read', 'policy.fresh_read')
       AND r.transition IS NOT NULL AND r.n > d.round)
       AS readings
  FROM policy_draft d;

CREATE VIEW observations_all AS
  -- Every observation of each project as the owner's page shows it: closed or not and why (one an item took in is
  -- closed once that item's work merged), with when it was recorded and last changed, as changes has it.
  -- Read by: the owner's interface; observations_open; the engine, for the overview the architect is given.
  SELECT o.project, o.id, o.what, o.tags, o.times_reported, o.source, o.refs, o.folded_into,
    CASE WHEN o.closed IS NOT NULL THEN o.closed WHEN m.merged IS NOT NULL THEN 'folded' END AS closed,
    CASE WHEN o.closed IS NOT NULL THEN o.closed_why
         WHEN m.merged IS NOT NULL THEN 'Item ' || o.folded_into || ' took it in, and its work merged' END AS closed_why,
    (SELECT min(c.at) FROM changes c WHERE c.table_name = 'observations' AND c.row_key = json_object('id', o.id))
      AS created,
    (SELECT max(c.at) FROM changes c WHERE c.table_name = 'observations' AND c.row_key = json_object('id', o.id))
      AS updated,
    m.merged
  FROM observations o LEFT JOIN (
    SELECT w.item, max(r.ended_at) AS merged FROM rounds r JOIN works w ON w.id = r.work
    WHERE r.cell IN ('M2', '9', 'merge_queue', 'merge.queue')
      AND r.transition = 'merged' GROUP BY w.item) m ON m.item = o.folded_into;

CREATE VIEW observations_open AS
  -- Each project's open observations, each with its number, what it is, its tags, how many times it was reported, where
  -- it came from, what it refers to and the item that took it in, if one has. Read by: cell `X2`, which searches
  -- them; the engine.
  SELECT project, id, what, tags, times_reported, source, refs, folded_into
  FROM observations_all WHERE closed IS NULL;

CREATE VIEW conversation_entries AS
  -- Every entry of the owner's conversations, gathered from the tables that hold them: one conversation for each
  -- engineer's work (what its agents tell and ask the owner, the owner's answers and interjections, its asks of the
  -- architect and the advisor with their replies, and the engine's notices about it), one for each project's architect
  -- (the owner's words, what it tells and asks the owner, and the design cards of the project's works for the owner to
  -- rule on), one for each pair of projects' architects (their messages), and one of wf-agents itself (its faults, and
  -- what the engine tells the owner about no work).
  -- Hidden from agents.
  -- Read by: the owner's interface, which lists the conversations and shows each.
  -- conversation is work:<id>, architect:<project>, architects:<project>|<project> in order, or wf-agents. source is
  -- what the entry is (message, notice, ask, answer, interjection, words, question, reply, advice_ask, advice, card,
  -- ruling or between) and id its row in the table it comes from, for advice_ask and advice the round. sender and
  -- recipient are a line, the owner, the engine or wf-agents; for between, the projects. waiting: it waits for the owner
  -- (an ask not answered, a design card not ruled, a fault not read). pending: the owner's words not taken up yet.
  -- agents: it passes between agents.
  SELECT CASE WHEN w.graph = 'architect' THEN 'architect:' || w.project ELSE 'work:' || w.id END AS conversation,
    coalesce(w.project, i.project) AS project, 'message' AS source, m.id AS id,
    coalesce(s.line, CASE WHEN w.graph = 'architect' THEN 'architect' ELSE 'engineer' END) AS sender,
    'owner' AS recipient, m.text AS text, coalesce(r.ended_at, r.started_at) AS at, m.work AS work, m.round AS round,
    NULL AS ask, NULL AS design_card, m.read_at AS read_at, 0 AS waiting, 0 AS pending, 0 AS agents, NULL AS fault
  FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
  LEFT JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
  UNION ALL
  SELECT CASE WHEN n.fault IS NOT NULL OR w.id IS NULL THEN 'wf-agents'
              WHEN w.graph = 'architect' THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    CASE WHEN n.fault IS NULL THEN coalesce(w.project, i.project) END, 'notice', n.id,
    CASE WHEN n.fault IS NULL THEN 'engine' ELSE 'wf-agents' END, 'owner', n.text, n.at, n.work, NULL, n.ask, NULL,
    n.read_at, n.fault IS NOT NULL AND n.read_at IS NULL, 0, 0, n.fault
  FROM engine_notices n LEFT JOIN works w ON w.id = n.work LEFT JOIN items i ON i.id = w.item
  WHERE n.ask IS NULL OR n.fault IS NOT NULL
  UNION ALL
  SELECT CASE WHEN w.graph = 'architect' THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'ask', a.id,
    CASE WHEN w.graph = 'architect' THEN 'architect'
         ELSE coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
                        WHERE x.work = a.work AND x.n < a.round ORDER BY x.n DESC LIMIT 1), 'engineer') END,
    'owner',
    coalesce((SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(a.body, '$.asked') q, json_each(q.value, '$.value.points') p),
             (SELECT n.text FROM engine_notices n WHERE n.ask = a.id ORDER BY n.id LIMIT 1), ''),
    coalesce((SELECT min(n.at) FROM engine_notices n WHERE n.ask = a.id), r.started_at), a.work, a.round, a.id, NULL,
    NULL, a.answer IS NULL, 0, 0, NULL
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  UNION ALL
  SELECT CASE WHEN w.graph = 'architect' THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'answer', a.id, 'owner',
    CASE WHEN w.graph = 'architect' THEN 'architect'
         ELSE coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
                        WHERE x.work = a.work AND x.n < a.round ORDER BY x.n DESC LIMIT 1), 'engineer') END,
    coalesce(nullif(json_extract(a.answer, '$.text'), ''), json_extract(a.answer, '$.choice'), ''),
    coalesce(json_extract(a.answer, '$.at'), r.started_at), a.work, a.round, a.id, NULL, NULL, 0, 0, 0, NULL
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  WHERE a.answer IS NOT NULL AND coalesce(json_extract(a.answer, '$.by'), 'owner') <> 'engine'
  UNION ALL
  SELECT CASE WHEN w.graph = 'architect' THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'interjection', j.id, j.speaker, j.line, j.text,
    coalesce((SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)),
             (SELECT min(x.started_at) FROM rounds x WHERE x.work = j.work), 0),
    j.work, j.delivered_round, NULL, NULL, NULL, 0, j.speaker = 'owner' AND j.delivered_round IS NULL,
    j.speaker <> 'owner', NULL
  FROM interjections j JOIN works w ON w.id = j.work LEFT JOIN items i ON i.id = w.item
  UNION ALL
  SELECT 'architect:' || m.project, m.project, 'words', m.id, 'owner', 'architect', json_extract(m.body, '$.text'),
    m.at, json_extract(m.body, '$.work'), NULL, NULL, NULL, NULL, 0, m.handled_round IS NULL, 0, NULL
  FROM architect_matters m
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.text') IS NOT NULL
  UNION ALL
  SELECT 'work:' || json_extract(m.body, '$.work'), m.project, 'words', m.id, 'owner', 'architect',
    json_extract(m.body, '$.text'), m.at, json_extract(m.body, '$.work'), NULL, NULL, NULL, NULL, 0,
    m.handled_round IS NULL, 0, NULL
  FROM architect_matters m
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.text') IS NOT NULL
    AND json_extract(m.body, '$.work') IS NOT NULL
  UNION ALL
  SELECT 'work:' || m.work, m.project, 'question', m.id,
    coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
              WHERE x.work = m.work AND x.n <= m.round ORDER BY x.n DESC LIMIT 1), 'engineer'),
    'architect', m.what, m.at, m.work, m.round, NULL, NULL, NULL, 0, 0, 1, NULL
  FROM architect_matters m JOIN works w ON w.id = m.work
  WHERE w.graph = 'engineer'
  UNION ALL
  SELECT 'work:' || m.work, m.project, 'reply', m.id, 'architect',
    coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
              WHERE x.work = m.work AND x.n <= m.round ORDER BY x.n DESC LIMIT 1), 'engineer'),
    coalesce(json_extract(m.reply, '$.text'), ''),
    coalesce((SELECT x.ended_at FROM rounds x WHERE x.work = m.handled_work AND x.n = m.handled_round), m.at),
    m.work, m.round, NULL, NULL, NULL, 0, 0, 1, NULL
  FROM architect_matters m JOIN works w ON w.id = m.work
  WHERE w.graph = 'engineer' AND m.reply IS NOT NULL
  UNION ALL
  SELECT 'work:' || r.work, i.project, 'advice_ask', r.n, coalesce(s.line, 'engineer'), 'advisor',
    coalesce((SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(r.output, '$.outcome.ask.points') p), ''),
    coalesce(r.ended_at, r.started_at), r.work, r.n, NULL, NULL, NULL, 0, 0, 1, NULL
  FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
  WHERE w.graph = 'engineer' AND r.transition = 'ask_advisor'
  UNION ALL
  SELECT 'work:' || r.work, i.project, 'advice', r.n, 'advisor', 'engineer',
    coalesce(json_extract(r.output, '$.outcome.reply.decision'), ''), coalesce(r.ended_at, r.started_at), r.work, r.n,
    NULL, NULL, NULL, 0, 0, 1, NULL
  FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item JOIN sessions s ON s.id = r.session
  WHERE w.graph = 'engineer' AND s.line = 'advisor' AND json_extract(r.output, '$.outcome.reply') IS NOT NULL
  UNION ALL
  SELECT 'architect:' || i.project, i.project, 'card', c.id, coalesce(s.line, 'engineer'), 'owner', '',
    coalesce(cr.ended_at, cr.started_at), c.work, c.round, NULL, c.id, NULL, c.status = 'open', 0, 0, NULL
  FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
  JOIN rounds cr ON cr.work = c.work AND cr.n = c.round LEFT JOIN sessions s ON s.id = cr.session
  WHERE c.status <> 'replaced'
  UNION ALL
  SELECT 'architect:' || i.project, i.project, 'ruling', c.id, 'owner', 'engineer', '',
    coalesce(json_extract(x.value, '$.at'), cr.ended_at, cr.started_at), c.work, c.round, NULL, c.id, NULL, 0, 0, 0,
    NULL
  FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
  JOIN rounds cr ON cr.work = c.work AND cr.n = c.round, json_each(c.rulings) x
  WHERE c.status <> 'replaced' AND json_extract(x.value, '$.by') = 'owner'
  UNION ALL
  SELECT 'architects:' || min(json_extract(m.body, '$.from'), m.project) || '|'
      || max(json_extract(m.body, '$.from'), m.project),
    NULL, 'between', m.id, json_extract(m.body, '$.from'), m.project, coalesce(json_extract(m.body, '$.text'), m.what),
    m.at, json_extract(m.body, '$.work'), NULL, NULL, NULL, NULL, 0, 0, 0, NULL
  FROM architect_matters m
  WHERE m.kind = 'other_architect' AND json_extract(m.body, '$.from') IS NOT NULL;
