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

CREATE TABLE migrations (
  -- Each migration the database has taken, by its file in db/migrations. A database made from this file has taken
  -- every one there is; one opened by a wf-agents with migrations it has not taken takes them, in the order their names
  -- sort, and its rows here name them. A migration is named by the minute it was written and what it does, so that
  -- works written side by side never take the same name, and these rows, not a count, say which a database took: a
  -- migration merged after one whose name sorts after it is taken after that one where that one came first. A database
  -- from before this table took the first migrations, as many as its version (`user_version`) less one.
  -- Hidden from agents.
  -- Read by: the engine, as it opens a database, to take the migrations it has not and to refuse one that took a
  -- migration it does not know, which a newer wf-agents made.
  -- The migration's file's name without `.sql`, as `0079` or `202611040140-record-migrations-taken`.
  name TEXT PRIMARY KEY
) STRICT;

CREATE TABLE items (
  -- A piece of work to do, as its creator wrote it. Text that a work's rounds produce later (the problem as the
  -- engineer understood it, the study, the design, progress) is not stored here: views read it from rounds. Items own
  -- nothing; how they depend on each other is in item_links. Only cancellation is stored as a status; every other
  -- status is derived.
  -- Read by: every cell that reads the item or the items around it; the engine when it starts and stops works.

  -- Item number, one sequence across all projects.
  id INTEGER PRIMARY KEY,
  -- The project the item belongs to. Picks the repository, CI and settings its works run with.
  -- Read by: the engine; cells `R1` and `D1`, through the project's design goals.
  project TEXT NOT NULL,
  -- What to do, as an action. Read by: cell `R1` for the items around this one; people.
  title TEXT NOT NULL,
  -- What cannot be done or is wrong, and why the work is worth doing, as written at creation. The engineer's own
  -- understanding replaces them in views once cell `R1` has run; these stay as written.
  -- Read by: cell `R1`, for this item and the items that depend on it.
  problem TEXT NOT NULL,
  why TEXT NOT NULL,
  -- How to do it, when the creator already knows; NULL otherwise. Read by: cell `R1`.
  approach TEXT,
  -- What the work hands in: a change to merge, or a report. Read by: cells `R1`, `D1` and `I1`; the engine.
  deliver TEXT NOT NULL CHECK (deliver IN ('change', 'report')),
  -- Agent level; decides which agent and model run the work. Read by: the engine at cell `S1`.
  level TEXT NOT NULL,
  -- The acceptance: a JSON list of acceptance rows in the flow's acceptance shape, as the research's plan or the design
  -- the owner approved gave them when the work went on to implement, and as the implementation, a fix or the advisor
  -- changed them since, each saying why; NULL until then. Read by: cells `I1`, `T2`, `T3`, `T1` and `T6`; the engine,
  -- which checks the conditions met against it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round that gave the acceptance, by its work and its number in the work: the research's, the design's, or the
  -- round of the implementation, a fix or the advisor that changed it. Read by: people following a condition back.
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
  -- there were none. Read by: cell `S1`, which takes them over when the item had no work here before; cell
  -- `R1`; the owner's interface.
  imported_branch TEXT,
  imported_pr INTEGER,
  -- The acceptance as the creator wrote it, JSON list of conditions; NULL when the creator wrote none. The acceptance
  -- the work goes by is acceptance. Read by: cell `R1`, through item_body.
  wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted)),
  -- Why the architect holds the item, with the owner's approval: no work starts on it while it is held; NULL while it
  -- is not. Read by: ready_items; the architect.
  held TEXT,
  -- Its place among its project's items in the architect's order, which works start in, lower first: the architect
  -- sets it with its approved changes, moving an item first, last, or before or after another item of the project,
  -- which ranks the project's items afresh. NULL for an item placed nowhere yet, as one just made is, which comes after
  -- those placed, in the order made. Read by: the dispatch order in ready_items; the architect.
  rank INTEGER,
  -- The decision points the item carries, JSON list in the flow's decision point shape, each with the option the owner
  -- chose and why: what a discussion settled that goes into the project's decisions/ once deep research confirms it.
  -- NULL when it carries none. An item that carries one goes to deep research, whatever its research finds. The
  -- architect writes them as it creates or edits the item. Read by: the engine's value `decision_points`; cells `R1`,
  -- `D1` and `T2`, through item_body.
  decision_points TEXT CHECK (decision_points IS NULL OR json_valid(decision_points)),
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
  -- Read by: the engine at cell `S0`; the architect, which is given it with the items and asks the owner where its
  -- order differs.
  item INTEGER PRIMARY KEY REFERENCES items(id),
  -- Lower starts first. A real number, so an item can be put between two others without renumbering.
  position REAL NOT NULL UNIQUE
) STRICT;

CREATE TABLE design_cards (
  -- A decision card: one question put to the owner, in the shape of the reasoning system's card extension, a revision
  -- of the project's records that the work's branch makes. A work has a card for each question; the owner rules each.
  -- A card is handed in by one round and belongs to that round; its item is found through the round's work. A ruled
  -- card never changes: the same question comes back as a new card under the same key that replaces it, and a question
  -- no longer asked is withdrawn, saying why. A card of a flow version before keys held a design card's decisions; it
  -- stays readable as it was. Read by: cells `R1`, `D1` and `D4` (the cards of this item, with their rulings), `D4`
  -- (the parent's current cards), `T4` and `T5` (the work's cards as they stand), `M1` (whether every current card is
  -- confirmed); the engine, which gives `H8` the card the owner asks about.
  id INTEGER PRIMARY KEY,
  -- The round that handed the card in: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The card's key, which stays the same for the same question in every later version of it in the work, so that
  -- rulings follow it; card-<id> for a card of a flow version before keys. Read by: the engine, which compares a card
  -- handed in with the work's current card of its key.
  key TEXT NOT NULL,
  -- The card as the round handed it in, JSON, in the flow's decision card shape. Copied from that round's output,
  -- which never changes, so the copy cannot drift.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The records the card revises, JSON list, as the engine read them on the branch when it posted the card: each its
  -- reference, whether the branch added, changed or removed it, and its text at the branch's head, or at the fork point
  -- for one removed; [] for a card of a flow version before keys. Read by: cells `T4` and `T5`; the owner's interface,
  -- which shows each with its words; `M1`, which holds the records to merge to the words the owner ruled on.
  revision TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(revision)),
  -- open: waiting for a ruling. ruled: the owner ruled it. replaced: a newer version of its key took its place, or its
  -- work ended, and it can no longer be ruled. withdrawn: the work no longer asks its question. Read by: the engine;
  -- cell `M1`.
  status TEXT NOT NULL CHECK (status IN ('open', 'ruled', 'replaced', 'withdrawn')),
  -- The rulings in the order given, JSON list: who ruled and when, the verdict (confirm, choose another option, or
  -- disagree), the option chosen and the owner's words. A ruling on a card of a flow version before keys holds a
  -- verdict on each decision of it by its number, or one verdict on the whole card. Read by: cells `R1`, `D1`, `D4` and
  -- `M1`.
  rulings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(rulings)),
  -- The card that replaced this one, the newer version of its key. Read by: people following a question to its latest
  -- card.
  replaced_by INTEGER REFERENCES design_cards(id),
  -- For a card that replaces an earlier version of its key: what changed from it and why, in the owner's language, as
  -- the round that handed it in wrote it; NULL for a key's first card. Read by: the owner's interface, beside the card it
  -- replaced.
  changed TEXT,
  -- Why the work no longer asks the card's question, in the owner's language, as the round that withdrew it wrote it;
  -- set only on a withdrawn card. Read by: the owner's interface.
  withdrawn TEXT,
  -- For the card that carries the flow of a work of wf-agents' own repository whose branch changes the flow: the flow
  -- as the branch had it when the card was posted, the flow version in force then, which the page compares it with,
  -- and the git object of the flow file the branch had, which the merge compares with the branch's; the object alone,
  -- with no flow, when it did not compile. Read by: the owner's interface, which draws it; cell `M1`, which has the card
  -- posted again when the branch's flow changed since.
  flow_snapshot INTEGER REFERENCES flow_snapshots(id),
  flow_in_force INTEGER REFERENCES flow_versions(id),
  flow_source TEXT,
  -- A replaced card names its successor, unless its work ended without one.
  CHECK (replaced_by IS NULL OR status = 'replaced'),
  CHECK ((withdrawn IS NOT NULL) = (status = 'withdrawn')),
  CHECK ((flow_snapshot IS NULL) = (flow_in_force IS NULL)),
  CHECK (flow_snapshot IS NULL OR flow_source IS NOT NULL),
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
  -- The owner's answer, JSON: who answered and when, and the text, and who the caller declared to speak (`speaker`,
  -- owner or delegate; none for an answer kept before it was declared); for an ask of the architect, the rest of the
  -- architect's reply too. NULL while it waits. Read by: the engine; cells `R1` and `L2`.
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
  -- For the answer to the owner's question about a decision card: that card, which the group shows the answer as a
  -- reply to. NULL for anything else.
  design_card INTEGER REFERENCES design_cards(id),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE engine_notices (
  -- What the engine tells the owner of its own accord, one row for each notice: a work stopped by a new flow version,
  -- every work halted until a usage limit resets, a limit passed, a fault of wf-agents, the asks still waiting for the
  -- owner's answer, and the like. It needs no answer. The same text goes to the notifications log, which alone also
  -- says what the page shows in a place of its own (a design card waiting, a work ready to merge but held).
  -- Hidden from agents.
  -- Read by: the owner's interface, which shows a fault in wf-agents' own conversation and what else is about no work
  -- there too, and says what is about a work on the bubble of the stage it was at, never as an entry of a work's group;
  -- the engine, for a fault of wf-agents (fault) and for when it last reminded the owner of the asks waiting
  -- (reminder).
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

CREATE TABLE contents (
  -- Each large text a round's record holds, once: a conditional input given in a message, a cell's task, the
  -- structured-output schema of a command line. The record refers to it by the SHA-256 of the text, as
  -- {"content": hash}, and the text it was is that record with each reference put back. Written once, never changed.
  -- Read by: the engine and the owner's interface, putting a round's record back together.
  hash TEXT PRIMARY KEY,
  text TEXT NOT NULL
) STRICT;

CREATE TABLE flow_versions (
  -- Every compiled flow a work has run on. A work runs on the version it started with until the flow moves to a new
  -- version; then, when a work on an older one next goes on, it moves to the new one at its cell when that cell and
  -- the shape of every return it handed in are unchanged there, and otherwise it stops and its item starts again on
  -- the new one. Old rounds stay readable after the flow changes.
  -- Read by: the engine; people and agents reading old rounds.
  id INTEGER PRIMARY KEY,
  -- Hash of compiled, so each version is stored once.
  digest TEXT NOT NULL UNIQUE,
  -- The compiler's expanded flow, JSON: every graph's cells, inputs, return schemas and transition rows. It is a
  -- megabyte or more, so a view does not parse it for each row it reads: what a round took from it is kept on the
  -- round (`rounds.counts`).
  compiled TEXT NOT NULL CHECK (json_valid(compiled))
) STRICT;

CREATE TABLE flow_snapshots (
  -- The flow as a work's branch has it, compiled when the engine posted a decision card of a work whose branch changes
  -- the flow, so that the owner rules a change to the flow having seen it drawn. Written once and never changed: the
  -- card's flow is drawn from here after the branch is merged or gone. Read by: the owner's interface.
  id INTEGER PRIMARY KEY,
  -- Hash of compiled, so each flow is stored once.
  digest TEXT NOT NULL UNIQUE,
  -- The compiler's expanded flow, JSON, as a flow version holds it.
  compiled TEXT NOT NULL CHECK (json_valid(compiled))
) STRICT;

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
  -- How the work ended: `done` when it reached the end of its graph, written in the write that completes the round at
  -- its end cell; otherwise why it was stopped before its end, for example because its item was cancelled. NULL while
  -- it goes on, a work waiting for a change to take effect included. Read by: the engine, which takes no step of a work
  -- that ended and does not load it; the status view.
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

CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session's context is past the configuration's limit, when the model its rounds run on
  -- changed, whose prompt cache it would have to write again whole, when the agents' login account changed, whose
  -- configuration directory of the CLI does not hold the session, when the CLI's prompt cache let it go (a line the
  -- flow says expires with the cache, its session last called a cache lifetime or more ago), and when the owner asks
  -- for one. Each project's architect's base, the session that read the project last, goes on in one CLI session across
  -- the works of the project's architect: each has a row of its own with the same cli_session. The matters of
  -- one run of the architect are handled in one session forked from the base: the CLI copies the base's session under a new
  -- id, so that it holds all the base read, and the fork's rounds are read after the base's. A cell on no line has a
  -- session of its own for each round, ended once the round has.
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
  -- session before; carried, a work of a project's architect going on in the CLI session the project's previous
  -- architect work left; account, the line's rounds run under another login account of the CLI than its session
  -- before, in another configuration directory, which does not hold that session; expired, the CLI's prompt cache
  -- let the line's session go, its last call a cache lifetime or more before, on a line the flow says expires with
  -- it; owner, the owner asked for a new session with the round's Start or Step on the page; fork, a first round on a
  -- line that forks from another, forked from that line's latest session, the base. Read by: the owner's
  -- interface.
  started TEXT NOT NULL DEFAULT 'line'
    CHECK (started IN ('line', 'cell', 'limit', 'model', 'carried', 'account', 'expired', 'owner', 'fork')),
  -- The session it was forked from, the base, for a session started as fork: it holds what the base held when it was
  -- forked, and its rounds are read after the base's. NULL for any other. Read by: the session manager, to tell what a
  -- fork holds and what it cost.
  fork_of INTEGER REFERENCES sessions(id),
  -- The matter of a project's architect the session is for, on a line whose sessions are each a matter's: a matter
  -- parked by a question to the owner takes its session up again when the answer comes, in whichever work of the
  -- architect runs then. NULL on any other line, and on the line whose session a work's matters share. Read by: the
  -- session manager, to find a matter's session.
  matter INTEGER REFERENCES architect_matters(id),
  CHECK ((started = 'fork') = (fork_of IS NOT NULL))
) STRICT;

CREATE TABLE pings (
  -- Each keep-warm ping: one minimal message the session manager sent an idle session of a line its work could come
  -- back to, 55 minutes after the session's last call, so that the session's prompt cache was read before it expired
  -- instead of written again whole at the line's next round. A ping means the line waited over an hour, while its work
  -- waited or went on without it: it says where the work stood and for what. Its cost is the work's. A project's
  -- architect's base, which the matters' sessions and the second eyes fork, is pinged so while its architect has a work
  -- running, and once before a fork is made when the cache may not hold the base's end under the fork's command line:
  -- after the base was built, after an install, or an hour after its last call; a fork holds the pings its base was
  -- sent before the fork was made.
  -- Hidden from agents.
  -- Read by: the session manager, which counts a wait's pings and a session's cost, and tells what the cache last kept
  -- a session under; the owner's interface and the anomalies command.
  id INTEGER PRIMARY KEY,
  session INTEGER NOT NULL REFERENCES sessions(id),
  -- Where the work stood and for what the line waited, JSON: the cell, what the work waited on (pause, slot, owner,
  -- architect, items or row) or work when it went on without the line, or fork when the ping warmed an architect's
  -- base before a fork of it, and that in words.
  waited TEXT NOT NULL CHECK (json_valid(waited)),
  -- The model the ping ran on, as the CLI reported it.
  model TEXT,
  -- Tokens and cost as the CLI reported them, JSON, with the ping's own cost as a round's usage has it; NULL while it
  -- runs.
  usage TEXT CHECK (usage IS NULL OR json_valid(usage)),
  -- How many times the CLI compacted the session during the ping: the session then holds nothing from before but its
  -- system prompt.
  compactions INTEGER NOT NULL DEFAULT 0 CHECK (compactions >= 0),
  -- Why the run failed, when it did.
  error TEXT,
  -- When it started and ended.
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  -- The call the ping ran, JSON, as a round's record keeps one (`rounds.input.call`), each large text kept in contents:
  -- for a ping of an architect's base, which runs the command line its forks run with. NULL for a ping that ran its
  -- session's last round's command line again, whose call is that round's. Read by: the session manager, which tells
  -- whether the cache keeps the base's end under a fork's command line, and compares a round that missed the cache with
  -- the call the cache kept its session under last.
  call TEXT CHECK (call IS NULL OR json_valid(call))
) STRICT;

CREATE TABLE cache_misses (
  -- Each round that missed its session's prompt cache: it resumed the session within the cache's lifetime since the
  -- session's last call, a round's or a keep-warm ping's, and its first call still read from the cache less than half
  -- of what the session held, the last prompt of its round before as recorded. A miss is a fault of wf-agents,
  -- whichever project it came up in: the engine tells the owner in a notice of the fault, and a further miss with the
  -- same cause on the same line of the same project, while that notice is unread or a matter forwarded from it waits
  -- in an architect's queue, adds to it instead of making a new one. A miss after what comes before the session's
  -- messages in the prompt changed since its round before (the model, the tools the envelope's schema is given with,
  -- the command line's own version), as when a work moved to a new flow version, is the command line writing the
  -- session again by design: no fault, kept for what it cost, with no notice. Written once, never changed.
  -- Hidden from agents.
  -- Read by: the engine, which finds the notice a miss adds to, and hands the misses of a fault forwarded to an
  -- architect to it; the owner's interface, which shows with each notice how many misses it holds, the tokens they
  -- wrote again and what they cost, and counts the misses that are faults among what was spent for nothing.
  -- The round that missed: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The line of its session.
  line TEXT NOT NULL,
  -- The cause as far as the record tells it, JSON: each part of the command line and each variable of its environment
  -- that differs from the session's round before, or from the keep-warm ping since it that recorded its call (as one
  -- that warmed an architect's base for a fork), with its value there and in this round; an empty list when none
  -- differs.
  differs TEXT NOT NULL CHECK (json_valid(differs)),
  -- The tokens of what the session held that the round's first call did not read from the cache, and so wrote to it
  -- again.
  written INTEGER NOT NULL CHECK (written >= 0),
  -- The tokens the session held: the last prompt of its round before, as recorded.
  held INTEGER NOT NULL CHECK (held > 0),
  -- What writing them again cost beyond reading them, in US dollars, priced from what the round cost; NULL when its
  -- record does not tell.
  cost_usd REAL,
  -- The notice that tells the owner of it; NULL for a miss the engine expects, which is no fault.
  notice INTEGER REFERENCES engine_notices(id),
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

CREATE TABLE owner_choices (
  -- The owner's settings on the page that hold across restarts of the engine and pick one of several, which a switch
  -- (owner_switches) cannot hold: one row for each the owner set; a setting with no row is as it is at first. Who set
  -- one, and when, is in changes, a setting put back as it is at first being its row deleted.
  -- Hidden from agents.
  -- Read by: the engine; the owner's interface.
  -- forward_faults: the project whose architect each new fault of wf-agents is forwarded to as it comes, as a matter
  -- of its queue, the notice then read; with no row, nobody's, and the owner forwards a fault by hand or not at all.
  name TEXT PRIMARY KEY CHECK (name IN ('forward_faults')),
  -- What the owner picked: for forward_faults, a project of the configuration.
  value TEXT NOT NULL
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

CREATE TABLE round_calls (
  -- The ledger of the agents' calls to the model: one row for each call a running round makes, as its command line's
  -- stream tells of it, however many lines of the stream give the call. The row is written when the stream first tells
  -- of the call and its output tokens follow as the stream tells them, so that it holds what a round used even when the
  -- round is stopped before the command line's report at its end, and how the context moved from call to call, which
  -- that report does not tell. A round from before the ledger has none.
  -- Hidden from agents.
  -- Read by: the run guard, for the session's last prompt when a round starts; the owner's interface, for the rounds
  -- running now.
  -- The round: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The call's place among the round's, from 1, in the order the stream first told of each.
  seq INTEGER NOT NULL CHECK (seq >= 1),
  -- The id of the message the call wrote, which tells a call apart however many lines give it.
  message TEXT NOT NULL,
  -- The model it ran on, as the stream names it.
  model TEXT,
  -- Its input tokens: not read from the cache, read from it, and written to it for five minutes and for an hour.
  input INTEGER NOT NULL CHECK (input >= 0),
  read INTEGER NOT NULL CHECK (read >= 0),
  written_5m INTEGER NOT NULL CHECK (written_5m >= 0),
  written_1h INTEGER NOT NULL CHECK (written_1h >= 0),
  -- Its output tokens: the most the stream told.
  output INTEGER NOT NULL CHECK (output >= 0),
  -- 1 for a subagent's call, which is not in the context of the round's session.
  subagent INTEGER NOT NULL CHECK (subagent IN (0, 1)),
  -- When the stream first told of it.
  at INTEGER NOT NULL,
  PRIMARY KEY (work, round, seq),
  UNIQUE (work, round, message),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE round_measures (
  -- What each agent's round that made a call came to, by what the run guard's lines are computed from: one row for each
  -- round, written once when it ended, or put in from its record by the migration that brought the ledger. A round that
  -- went past a signal that stops a round has none, stopped or let go on by the owner's Start, so that an anomaly never
  -- becomes its own baseline.
  -- Hidden from agents.
  -- Read by: the run guard, which computes a round's lines from these rows when it starts.
  -- The round: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The work's graph and the round's cell, as the flow version it ran on names them.
  graph TEXT NOT NULL,
  cell TEXT NOT NULL,
  -- The model it ran on, as the command line reported it.
  model TEXT,
  -- Its calls to the model, those of subagents left out. Not from the ledger: the turns the command line reported, which
  -- may be more than its calls.
  calls INTEGER NOT NULL CHECK (calls >= 1),
  -- The tokens its context grew by: every rise between neighbouring calls added up, so that a compaction takes nothing
  -- back. Not from the ledger: its last prompt less its first, which a compaction hides.
  added INTEGER NOT NULL CHECK (added >= 0),
  -- The tokens its first prompt was past the last prompt of its session before it; NULL for a session's first round.
  jump INTEGER CHECK (jump IS NULL OR jump >= 0),
  -- The characters the engine gave it: its message and the return before it in its session; NULL when its record does
  -- not tell.
  given INTEGER CHECK (given IS NULL OR given >= 0),
  -- The tool uses that repeated an earlier one of the round exactly; NULL when not from the ledger, since nothing kept
  -- tool uses before it.
  repeats INTEGER CHECK (repeats IS NULL OR repeats >= 0),
  -- How many times the command line compacted the session during it.
  compactions INTEGER NOT NULL CHECK (compactions >= 0),
  -- What it cost, in US dollars; NULL when its record does not tell.
  cost_usd REAL,
  -- 1 when kept call by call as it ran, 0 when put in from its record. A cell's own line reads only the former.
  ledger INTEGER NOT NULL CHECK (ledger IN (0, 1)),
  PRIMARY KEY (work, round),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE guard_events (
  -- Each signal of a running round that went past its line, once in a round: what the run guard saw and did, kept as
  -- every anomaly is. Written once.
  -- Read by: the owner's interface, which lists them under anomalies; agents and the architect tracing what the guard
  -- did to a round.
  id INTEGER PRIMARY KEY,
  -- The round: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The signal: growth, the context added in the round, with how much a call; jump, the first prompt past the session's
  -- last, with how much for each character the engine gave; cost; calls; repeats, tool uses repeated exactly;
  -- compactions.
  signal TEXT NOT NULL CHECK (signal IN ('growth', 'jump', 'cost', 'calls', 'repeats', 'compactions')),
  -- Its value and its line: tokens, US dollars or a count, as the signal is.
  value REAL NOT NULL,
  line REAL NOT NULL,
  -- For growth and jump, the second condition: its value and line. NULL for the signals of one condition.
  also REAL,
  also_line REAL,
  -- How many rounds gave the line, and whether they were the cell's own or the whole graph's.
  samples INTEGER NOT NULL CHECK (samples >= 1),
  scope TEXT NOT NULL CHECK (scope IN ('cell', 'graph')),
  -- What came of it: noted, recorded and the round went on; stopped, the round was stopped; waived, it would have been
  -- stopped, but the owner let the work go on after the cell's last round was stopped.
  action TEXT NOT NULL CHECK (action IN ('noted', 'stopped', 'waived')),
  -- When it went past.
  at INTEGER NOT NULL,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

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
  -- Each merge the engine reverted: the project's architect judged that it made main red and asked for its revert
  -- (the action `revert_merge`), and the engine opened a pull request that reverts it and merged it into main. Written
  -- once the revert is merged, never changed after.
  -- Read by: the health numbers, which count the reverts, and count a commit on main that is a revert apart from one
  -- that came onto it without the flow.
  -- The architect's work that asked for the revert, which says whose project it is.
  work INTEGER NOT NULL REFERENCES works(id),
  -- The merge reverted, and the commit the revert's merge put on main, each by its full id.
  merge_commit TEXT NOT NULL,
  revert_commit TEXT NOT NULL UNIQUE,
  -- When the revert was merged into main.
  at INTEGER NOT NULL
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
  -- For what the owner said to the engineer alone: who the caller declared to speak, owner, or delegate for a session
  -- that speaks for the owner; NULL for what the architect said, for what the engine wrote in the owner's name (a
  -- ruling on a design card) and for words kept before it was declared. Read by: the engine, which tells who decided a
  -- decision that quotes the words.
  declared TEXT CHECK (declared IN ('owner', 'delegate')),
  -- For what a delegate said to the engineer alone and declared to hold the owner's own words: those parts, a JSON array
  -- of strings, each found letter for letter in the words (text); NULL when none was declared. Read by: the engine,
  -- which tells a decision that quotes one of the parts as the owner's, relayed by the delegate, not checked.
  relayed TEXT CHECK (relayed IS NULL OR (json_valid(relayed) AND declared = 'delegate')),
  -- Their words as said.
  text TEXT NOT NULL,
  -- The round of the work it was handed in, by its number in the work; NULL until then. Read by: the engine, to find
  -- what is still to hand in.
  delivered_round INTEGER,
  -- For the notice that an item this work's item depended on was cancelled: that item; NULL for anything else said.
  -- The cells where the work waits (`A2`, `A6`, `M1`) take such a notice to choose the way on by a row of their own,
  -- as an item they waited for that will never deliver, when it is for the line the work stands on: a question asked
  -- from another line is not withdrawn for it, since the cell the work goes back to would not hear it. Read by: the
  -- engine's values `cancelled` and `held`.
  cancelled_item INTEGER REFERENCES items(id),
  -- 1 for a word that needs no answer and changes no way on, as the owner confirming every decision of a design card:
  -- the line's next round is given it, and a work standing at an engine step is not taken by it to choose the way on.
  -- Read by: the engine's values `interjection`, `cancelled` and `held`, which leave such words out.
  informs INTEGER NOT NULL DEFAULT 0 CHECK (informs IN (0, 1)),
  -- For what the owner said in the work's group to its project's architect: that architect's matter, which holds the
  -- words. Said to the architect alone, the engineer is given it as a word that needs no answer (informs); said to
  -- both, as said to it. NULL for anything said to the engineer alone. Read by: the owner's interface, which shows the
  -- words once, to whom they were said.
  matter INTEGER REFERENCES architect_matters(id),
  -- For the owner's question about a decision card, asked on the card in the work's group: that card, which the
  -- engineer is given with the words, and the group shows them as a reply to. NULL for anything else said.
  design_card INTEGER REFERENCES design_cards(id),
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
  -- Read by: people and agents looking at how something got where it is; views that need when a row was created, said
  -- or came to a state, or in what order rows did (find them by searching this file for `FROM changes`). Nothing
  -- deletes from it: such a view would answer differently if rows were cut away, so cutting it first means changing
  -- those views.
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

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, main's CI red, a place its plan of what to do next found to set straight, or the
  -- check of its policy. The project's queue is its matters not handled; the engine takes them in a fixed order, by
  -- what waits on each: the owner's words and replies first, then main's CI red, then what a work stopped waits on, the
  -- one that came first; then the base when it is due; then what the plan queued; then what waited for the planning.
  -- Each is handled in a session of its own forked from the base. A matter something waits on starts a work of the
  -- project's architect when none runs; one that waits for the planning starts one only once it came before the base
  -- was built, or once as many wait as the installation's amount.
  -- Read by: the engine, which starts the architect's works, takes each matter, routes it to its path, hands it its
  -- material and takes its replies back; the architect's cells through architect_queue and work_handled; the owner's
  -- interface.
  id INTEGER PRIMARY KEY,
  -- The project whose architect it is for.
  project TEXT NOT NULL,
  -- Its kind, which decides the path it takes: set_straight, engineer_question, open_question, report, design_card,
  -- failure, owner_words, other_architect, slots_free, main_red (main's CI red, which the engine's watch of main
  -- queues: its body holds the commit, its red checks and their runs, and while one is open the project's merge queue
  -- is stopped), or check_policy, the check of the policy in force, one open at a time; slots_free and make_plan (the
  -- plan of what to do next, which a flow version before queued) are what a planning answers. An open question is what an engineer's design could not settle: its body holds the question, the options, the
  -- one recommended, the link to the design (design_link) and, once it has come, the answer (answer) with who gave it
  -- (answered_by: architect, or owner when the architect pinned the question for the owner and the owner answered), for
  -- the owner the ask that holds their words (owner_ask), and the time it came (answered_at, in milliseconds). A
  -- matter handled before these kinds keeps the kind it had then: set_policy for the setting of a new policy, which a
  -- flow version before ran as a stage of its own, and policy for a setting that ended, when the setting was a work of
  -- its own.
  kind TEXT NOT NULL,
  -- What more its kind says: an engineer's question's own kind (split, duplicate, ...); a report's (a side report's
  -- kind, followup for a report's what to do next, answer for an engineer's answer to what the architect said to its
  -- work, flow_feedback for what agents said was wrong with their cells, or fault for a fault of wf-agents forwarded
  -- to it, by the owner or by the owner's setting, which starts a work of it as a report does); a failure's
  -- (tool_failed, environment_changed); the owner's words' (said, answer, release, approved_changes); a message's
  -- (message, reply, or flow_feedback for flow feedback another project's architect forwarded); the kind of a place to
  -- set straight. NULL when there is nothing more. Read by: the engine, for the matter's material and whether it starts a work of
  -- the architect; the owner's interface.
  detail TEXT,
  -- What it is, in one sentence, as the engine wrote it.
  what TEXT NOT NULL,
  -- Its material, JSON: what the asking cell handed in, the side report, a report's what to do next with its paths and
  -- the commit merged, the design card, the tool that failed, the owner's words (with where they were said, and who
  -- the caller declared to speak: `speaker`, owner or delegate; none for words kept before it was declared), the
  -- message, the place to set straight; for a fault forwarded, who forwarded it (by: owner, or setting for the owner's
  -- setting), the fault itself being read from its notice when the matter is handled. Read by: the engine, for the
  -- matter's material.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The item it concerns; NULL when it concerns none.
  item INTEGER REFERENCES items(id),
  -- The round that raised it: an engineer's round at the architect's cell, a round that handed in a side report or a
  -- design card, an architect's round; NULL for one the owner or the engine raised.
  work INTEGER,
  round INTEGER,
  -- For a fault of wf-agents forwarded to it: the engine's notice that reports the fault, which goes to each
  -- project's architect once; NULL for any other matter. Read by: the engine, which hands the architect the notice
  -- and the engine's evidence of the fault as they stand when the matter is handled, and adds a further cache miss to
  -- a notice forwarded while its matter waits; the owner's interface, which shows where each fault went and how it
  -- stands there.
  notice INTEGER REFERENCES engine_notices(id),
  -- When it came. Read by: the queue, for how long it waited.
  at INTEGER NOT NULL,
  -- The ask of the owner a cell on its path put about it, which holds it while the owner has not answered: it is then
  -- out of the queue, and once answered it is back, and goes back to the cell that asked, with the answer. NULL when no
  -- cell asked about it. Read by: the queue; the engine, which routes it and gives the answer.
  asked INTEGER REFERENCES owner_asks(id),
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the approval of
  -- the draft of a new policy, the cell that wrote it, a reply of exactly approved going to where it takes effect
  -- instead.
  resume_cell TEXT,
  -- For a matter of two cells that its first cell sent back to the queue, as a flow version before did so that the
  -- owner's words that came meanwhile were taken first: its second cell, where it goes on (`H3` after `H2`, `H5` after
  -- `H4`, `P2` after `P1`). No cell sets it now, since a matter's cells follow one another in its own session; it is
  -- cleared once the matter is closed or a cell on its path asks the owner about it. Read by: the engine, which routes
  -- a matter that still holds one there.
  goes_on_at TEXT,
  -- For a side report or an engineer's answer a flow version before had the architect pick together with others of
  -- its kind in one pick, the matter that led the group: the one picked. No pick groups matters now, each being handled
  -- in a session of its own; it stays on a matter handled so, saying with which others it was handled. Read by: the
  -- owner's interface.
  group_lead INTEGER REFERENCES architect_matters(id),
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. The owner's answer in the
  -- work's group to a question the architect holds is the reply too, said by the owner (by), with who the caller
  -- declared to speak (speaker, owner or delegate). Read by: the engine, which takes it back unchanged to the work that
  -- asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the second eye, JSON, as the round handed it in: it becomes the reply, with the numbers of
  -- the items made, once the second eye finds the round's changes sound. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its work and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; the owner ruled the design card it was to look at, or another card replaced it, saying which;
  -- the owner answered the question in the work's group, the reply saying what; it said that a setting of the
  -- policy, a work of its own then, had ended, when the setting became a stage of the architect's works; or it was a
  -- setting of the policy as a stage of its own, which runs so no longer. NULL otherwise.
  -- Read by: architect_queue, which leaves it out; the engine, which tells a new work on the item what an earlier
  -- work asked and had withdrawn; the owner's interface, which leaves it out of the queue.
  withdrawn TEXT,
  -- What the session that handled it left as it ended, JSON: its record (`record`), and the lines it adds to the plan
  -- (`plan`) and to the understanding (`understanding`) when it adds any, with the round that handed them in (`round`).
  -- NULL until then, and for a matter handled before the architect's matters were forked. Read by: the engine, which
  -- gives every matter handled after it what came since the base, and the next plan what came since the plan before;
  -- the owner's interface.
  logged TEXT CHECK (logged IS NULL OR json_valid(logged)),
  -- What the round handed in that waits for the second eye, JSON: its effects and its answer, made once the second eye
  -- finds the changes sound, or dropped when it sends them back. NULL otherwise. Read by: the engine.
  effects_held TEXT CHECK (effects_held IS NULL OR json_valid(effects_held)),
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE talk_entries (
  -- What the owner said to a project's architect in their one-to-one while a talk with the owner was open, as a flow
  -- version before held one (the architect answering at cell `H8` or waiting for the owner's reply at `H11`, or the
  -- talk held out of the queue and not ended): an entry of that talk, after the owner's words that started it, and no
  -- matter of its own. Nothing writes it now: every word the owner says to the architect is a matter of its own.
  -- Read by: the owner's interface, which shows them in the one-to-one; the check of a policy, which quotes none of the
  -- owner's words.
  id INTEGER PRIMARY KEY,
  -- The talk: the matter of the owner's words that started it.
  matter INTEGER NOT NULL REFERENCES architect_matters(id),
  -- What the owner said.
  text TEXT NOT NULL,
  -- When the owner said it.
  at INTEGER NOT NULL,
  -- The architect's turn of the talk that took it, by its work and round; NULL while no turn took it.
  taken_work INTEGER,
  taken_round INTEGER,
  CHECK ((taken_work IS NULL) = (taken_round IS NULL)),
  FOREIGN KEY (taken_work, taken_round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE blockers (
  -- What a stopped task waits on, one row for each stop: an engineer's question, an engine tool's failure, an ask of the
  -- owner, a wait for items to deliver, or a wait for the owner's ruling on a design card. A task is an engineer's work,
  -- or the setting of a policy in a flow version that ran it as a work of its own; a work of the architect's graph is
  -- never one. A blocker belongs to the task and the cell that raised it, and has exactly one
  -- holder at a time, who must act: the work's advisor, a project's architect, the owner, or the items it waits for,
  -- whose merge resolves it. A task stops on one blocker at a time, and stands at its holder's cell while it waits;
  -- every move of a blocker is a row of the task's graph, which the engine records here.
  -- Read by: the engine, which moves a blocker by the rows its task takes and checks that every chain closes; the
  -- owner's interface, which reads the owner's conversations and what each work waits for from it.
  id INTEGER PRIMARY KEY,
  -- The task that stopped.
  work INTEGER NOT NULL REFERENCES works(id),
  -- The cell that raised it, where its problem arose: the cell that asked, the tool cell whose tool failed, or the cell
  -- where the work waits for items or rulings; and the round of the task that raised it there, NULL for one recorded
  -- before blockers were.
  cell TEXT NOT NULL,
  round INTEGER,
  -- question: an engineer's question; tool_failure: an engine tool that failed, or main's environment that changed;
  -- owner_ask: an ask of the owner; items: a wait for items to deliver; ruling: a wait for the owner's ruling on the
  -- design card before the merge.
  kind TEXT NOT NULL CHECK (kind IN ('question', 'tool_failure', 'owner_ask', 'items', 'ruling')),
  -- What the problem is, in one sentence, as the engine wrote it; the return of the round that raised it holds the
  -- rest.
  what TEXT NOT NULL,
  -- Who holds it now: advisor, the work's advisor; architect, the architect of project; owner; items, the items it
  -- waits for, whose merge resolves it.
  holder TEXT NOT NULL CHECK (holder IN ('advisor', 'architect', 'owner', 'items')),
  -- For an architect that holds it, the project whose architect it is, the work's own or a related one; NULL for any
  -- other holder.
  project TEXT,
  -- For items that hold it, JSON list: each item waited for, with its project. NULL for any other holder.
  items TEXT CHECK (items IS NULL OR json_valid(items)),
  -- The matter in its architect's queue while an architect holds it, its latest one. Read by: the engine, which takes
  -- the architect's reply from it and withdraws it when the blocker ends.
  matter INTEGER REFERENCES architect_matters(id),
  -- The ask put before the owner while the owner holds it, its latest one. Read by: the engine, which takes the owner's
  -- answer from it and closes it when the blocker ends; the owner's interface.
  ask INTEGER REFERENCES owner_asks(id),
  -- When it was raised.
  at INTEGER NOT NULL,
  -- When its holder took it, or when its holder's wait was counted again: an architect's time to act on it is
  -- measured from here. Read by: the engine's value that says it waited past its limit.
  since INTEGER NOT NULL,
  -- Every hand-over, JSON list in order: when, from which holder to which (with the project of an architect), why, and
  -- what the holder that handed it on found.
  moves TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(moves)),
  -- What is said about it without resolving it or moving it, JSON list in order: when, by whom, and what, such as an
  -- architect's reply that waits for its batch's review, or a resolution that came after it was withdrawn, which is
  -- recorded here and moves nothing.
  said TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(said)),
  -- How it ended: resolved, by its holder; withdrawn, by its task's end or by a row that took the task on without it;
  -- NULL while it is open.
  ended TEXT CHECK (ended IN ('resolved', 'withdrawn')),
  -- For one resolved, what was decided and done, JSON, as its holder gave it; for one withdrawn, why. NULL while it is
  -- open.
  outcome TEXT CHECK (outcome IS NULL OR json_valid(outcome)),
  -- When it ended.
  ended_at INTEGER,
  CHECK ((holder = 'architect') = (project IS NOT NULL)),
  CHECK ((holder = 'items') = (items IS NOT NULL)),
  CHECK ((ended IS NULL) = (outcome IS NULL)),
  CHECK ((ended IS NULL) = (ended_at IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE architect_item_changes (
  -- A change to the items a project's architect handed in while handling a matter: made as its round ends, once the
  -- engine's checks found it sound, or, with the rest of a round that holds a change of a kind a second eye checks,
  -- once the second eye finds it sound; kept as the log of every change made, sent back or dropped. A flow version
  -- before collected the changes into a batch, made once a review of the whole batch found them sound.
  -- Read by: the engine, which gives the second eye the changes that wait for it and makes the changes; the owner's
  -- interface.
  id INTEGER PRIMARY KEY,
  -- The round that handed it in: an architect work's round on a matter's path.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The matter it came from.
  matter INTEGER NOT NULL REFERENCES architect_matters(id),
  -- The change, JSON, in the flow's item_change shape, with its reason.
  change TEXT NOT NULL CHECK (json_valid(change)),
  -- collected: waiting for the second eye (in a flow version before, for the review of its batch); made: the engine
  -- made it; revised: the second eye sent it back, and the matter's session handed in another in its place (in a flow
  -- version before, a revision of the batch took its place); dropped: it could not be made in the state the changes
  -- before it left, as a dependency that would close a cycle (in a flow version before, the review dropped it).
  state TEXT NOT NULL CHECK (state IN ('collected', 'made', 'revised', 'dropped')),
  -- For a change made, what it made, JSON, such as the number of the item it created; for one sent back, what the
  -- second eye found; for one dropped, why. NULL otherwise. Read by: the owner's interface.
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
  -- What is wrong, as a short phrase in the owner's language, at most `items.title_max` wide, spaces not counted and a
  -- character of East Asian wide or full width counting 2, as the architect wrote it when it recorded it; NULL for one
  -- recorded before observations had titles, which may be in English and of the width then allowed. Read by: the
  -- owner's interface, which leads each row of a list of observations with it.
  title TEXT,
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
  -- What stops new works of a project from starting, one row for each: the owner's hold, its architect's own hold, its
  -- architect setting it straight. While a project has a row here, the engine starts no new work on its items; works
  -- already running go on, and the other projects' works start as usual.
  -- Read by: the engine at cell `S0`; the architect; the owner's interface.
  project TEXT NOT NULL,
  -- owner: held by the owner. architect: held by the architect's own action. set_straight: its plan of what to do next
  -- found the whole must be set straight. The owner releases any of them; the architect only its own and the one to
  -- set straight.
  reason TEXT NOT NULL CHECK (reason IN ('owner', 'architect', 'set_straight')),
  -- When it began.
  since INTEGER NOT NULL,
  -- Why, in the owner's language, as the owner or the architect said it for their own hold; NULL for the one to set
  -- straight, whose reason says why. Read by: the owner's interface.
  why TEXT,
  PRIMARY KEY (project, reason)
) STRICT;

CREATE TABLE slot_waits (
  -- Each stretch of time a work waited for a free slot: what it waited for had come (the owner's or the architect's
  -- answer, or what the row it took waited for), and it waited on, holding no slot, until its machine had one free.
  -- The engine's loop opens a row as it reaps a step of the work that found its wait over and was granted no slot, and
  -- closes it as it grants the work a slot or finds the work ended, paused or gone on.
  -- Read by: the engine's loop, which grants the works with an open row a free slot before any new work starts; the
  -- owner's interface, through the engine's status, which names such a work as waiting for a free slot; whoever looks
  -- at why works stood still.
  id INTEGER PRIMARY KEY,
  -- The work that waited.
  work INTEGER NOT NULL REFERENCES works(id),
  -- When its wait for a slot began: when the step that found its wait over ended.
  since INTEGER NOT NULL,
  -- When its wait for a slot ended; NULL while it lasts.
  until INTEGER,
  -- How it ended: granted, the loop granted the work a free slot; ended, the work ended; paused, the work was paused;
  -- gone_on, the work went on without a slot, as by a round the owner asked for, or no longer waited where it had, as
  -- when it was brought to a newer flow version. NULL while it lasts.
  ended TEXT CHECK (ended IN ('granted', 'ended', 'paused', 'gone_on')),
  CHECK ((until IS NULL) = (ended IS NULL))
) STRICT;

CREATE TABLE engine_runs (
  -- Each time the engine's loop (`wf-agents run`) ran on this database, from its start to its stop, so that a gap in
  -- which no work moved can be told to be a stop and a start of the engine or not: when it started, which build and
  -- configuration it ran, and why it stopped. A row is written as the loop starts and completed as it stops. A start
  -- that finds a row never completed marks it lost: the engine's process ended without recording its stop (it was
  -- killed, or the machine went down), and when is not known. Two engines are not run on one database at once.
  -- Read by: the owner and the architect, through the tool `query`, looking at why works stood still; the engine
  -- reads it only to mark the rows a start finds never completed.
  id INTEGER PRIMARY KEY,
  -- When the loop started.
  started_at INTEGER NOT NULL,
  -- The commit the engine was built from, the one every round records as the engine's version.
  built_from TEXT NOT NULL,
  -- The commit of the configuration the loop started with. A committed change applies from the next round on, and each
  -- round says which commit it ran with.
  config_commit TEXT NOT NULL,
  -- The version the flow built into this build is stored as.
  flow_version INTEGER NOT NULL REFERENCES flow_versions(id),
  -- When the loop stopped; NULL while it runs, and for a row marked lost.
  stopped_at INTEGER,
  -- Why it stopped; NULL while it runs. `stop file`: the owner, or whoever installs a new build, asked it to stop, and
  -- it dispatched nothing more and waited for the jobs in flight to end. `error: ` and the text: the loop ended on an
  -- error it could not deal with. `lost`: the row was found never completed, as above.
  stop_reason TEXT,
  CHECK ((stop_reason IS NULL) = (stopped_at IS NULL) OR stop_reason IS 'lost')
) STRICT;

CREATE TABLE notebook_edits (
  -- Each version of a part of a project's architect's notebook, as a round of its base wrote it whole: its
  -- understanding of the project (project_now) and its plan (plan), in the owner's language; unclear and other are
  -- sections earlier flow versions wrote, kept as they were. A part is what its latest version says; the versions
  -- before are kept for the owner.
  -- A new base writes its understanding from nothing, and the next plan is given the plan before.
  -- Hidden from agents.
  -- Read by: the engine, which gives the next plan the plan before, through notebook; the owner's interface, which
  -- shows each part with its versions.
  id INTEGER PRIMARY KEY,
  -- The project whose architect's notebook it is.
  project TEXT NOT NULL,
  -- Which section.
  section TEXT NOT NULL CHECK (section IN ('project_now', 'plan', 'unclear', 'other')),
  -- The section's whole text in this version.
  text TEXT NOT NULL,
  -- The round of the architect's line that wrote it: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE conversation_reads (
  -- How far the owner has read each of the owner's conversations: the time of the latest entry the page showed while
  -- the owner had the conversation open. What came after it, from anyone but the owner, is unread.
  -- Hidden from agents.
  -- Read by: the owner's interface, which counts each conversation's unread entries.
  -- The conversation, by its key as the owner's interface names it (work:<id>, architect:<project>,
  -- projects:<project>|<project>..., wf-agents).
  conversation TEXT PRIMARY KEY,
  -- The time of the latest entry read.
  at INTEGER NOT NULL
) STRICT;

CREATE TABLE attachments (
  -- An image the owner sent with words: picked or pasted in a box of the owner's page and sent with the words written
  -- beside it, as one message, or alone. A PNG, JPEG, GIF or WebP image of at most 5 MB, at most 10 to a message. It
  -- is kept here, not in a file, so that it reaches an agent in its round's message and no agent has to find a file
  -- or be let read one: the engine gives an image to an agent's session with the first of the owner's words naming
  -- it (`image:<id>`) that the session is given, and refers to it after that. Written once with its words and never
  -- changed: who sent it, and when, is the change that wrote its words. Not tracked in changes, which holds no bytes.
  -- Hidden from agents.
  -- Read by: the engine, which gives each image with its words; the owner's interface, which shows the images under
  -- the owner's words and in the round that was given them.
  id INTEGER PRIMARY KEY,
  -- The words it came with, exactly one of: what the owner said to a work's line (an interjection); the owner's words
  -- an architect's matter holds (the words of a matter of the kind owner_words, or the owner's reply to an engineer's
  -- question a matter holds); or the owner's answer to an ask.
  interjection INTEGER REFERENCES interjections(id),
  matter INTEGER REFERENCES architect_matters(id),
  ask INTEGER REFERENCES owner_asks(id),
  -- Its place among the images of its words, from 1, in the order the owner added them.
  position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 10),
  -- What kind of image it is.
  media_type TEXT NOT NULL CHECK (media_type IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp')),
  -- Its size in bytes.
  size INTEGER NOT NULL CHECK (size BETWEEN 1 AND 5242880),
  -- The SHA-256 of its bytes, in hex. Read by: the engine, which tells by it whether a session holds the image.
  hash TEXT NOT NULL,
  -- The image itself.
  bytes BLOB NOT NULL CHECK (length(bytes) = size),
  CHECK ((interjection IS NOT NULL) + (matter IS NOT NULL) + (ask IS NOT NULL) = 1)
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
-- The rounds of a work that counted something, by the counter, held in the index alone: the views of counters read
-- them for one work without reading the rounds' large inputs and outputs.
CREATE INDEX rounds_counts ON rounds (work, counts) WHERE counts IS NOT NULL;
-- The rounds that started, and those that ended, within a time: the health numbers read a period by them.
CREATE INDEX rounds_by_start ON rounds (started_at);
CREATE INDEX rounds_by_end ON rounds (ended_at);
-- The merge by the flow, if any, whose commit is a given one: the health numbers tell a commit on main the flow merged
-- from one that came onto it without the flow by it. A statement uses it only when it writes the expression as here.
CREATE INDEX rounds_merge_commit ON rounds (json_extract(output, '$.result.merge_commit')) WHERE transition = 'merged';
-- The samples of a measure within a time, of every project together.
CREATE INDEX samples_by_time ON samples (measure, at);
-- The tools a round ran.
CREATE INDEX tool_runs_by_round ON tool_runs (work, round);
-- The rounds a cell's and a graph's lines are computed from.
CREATE INDEX round_measures_by_cell ON round_measures (graph, cell);
-- The decision cards a round handed in.
CREATE INDEX design_cards_by_round ON design_cards (work, round);
-- A work's current card of each key, at most one.
CREATE UNIQUE INDEX design_cards_current ON design_cards (work, key) WHERE status IN ('open', 'ruled');
-- The ask of the owner a round put.
CREATE INDEX owner_asks_by_round ON owner_asks (work, round);
-- The messages to the owner still unread.
CREATE INDEX owner_messages_unread ON owner_messages (id) WHERE read_at IS NULL;
-- Interjections still to hand in to a work.
CREATE INDEX interjections_waiting ON interjections (work) WHERE delivered_round IS NULL;
-- The history of one row.
CREATE INDEX changes_by_row ON changes (table_name, row_key);
-- The changes to a table within a time, which the health numbers read a period by.
CREATE INDEX changes_by_time ON changes (table_name, at);
-- The next action due.
CREATE INDEX outbox_by_run_after ON outbox (run_after);
-- The architect's matters not handled.
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
-- The matters of a kind that came within a time, which the health numbers read a period by.
CREATE INDEX architect_matters_by_kind ON architect_matters (kind, at);
-- The matters a round handled, which the health numbers read from the rounds that ended within a time.
CREATE INDEX architect_matters_by_handling ON architect_matters (handled_work, handled_round);
-- Where each fault of wf-agents was forwarded: to each project's architect once.
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;
-- The entries of each talk with the owner.
CREATE INDEX talk_entries_by_talk ON talk_entries (matter);
-- The open blocker of each task: one at most.
CREATE UNIQUE INDEX blockers_open ON blockers (work) WHERE ended IS NULL;
-- The blockers raised within a time, and those that ended after a time, which the health numbers read a period by.
CREATE INDEX blockers_by_time ON blockers (at);
CREATE INDEX blockers_by_end ON blockers (ended_at);
-- The open wait for a free slot of each work: one at most.
CREATE UNIQUE INDEX slot_waits_open ON slot_waits (work) WHERE until IS NULL;
-- The item changes of a batch still waiting for its review.
CREATE INDEX architect_item_changes_collected ON architect_item_changes (work) WHERE state = 'collected';
-- A project's observations.
CREATE INDEX observations_by_project ON observations (project);
-- A project's architect's notebook, by its sections.
CREATE INDEX notebook_edits_by_section ON notebook_edits (project, section);
-- The images each of the owner's words came with, in order: one in each place.
CREATE UNIQUE INDEX attachments_of_interjection ON attachments (interjection, position) WHERE interjection IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_matter ON attachments (matter, position) WHERE matter IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_ask ON attachments (ask, position) WHERE ask IS NOT NULL;
-- The largest transaction number in the change log: `Db::write` reads it on every write, under the write lock.
CREATE INDEX changes_by_tx ON changes (tx);
-- The sessions a CLI session's id names: the rows its rounds are read through.
CREATE INDEX sessions_by_cli_session ON sessions (cli_session);
-- The rounds a CLI session ran, found through the session's row.
CREATE INDEX rounds_by_session ON rounds (session);
-- The rounds of a cell in any work: an architect's rounds at the cell of a matter.
CREATE INDEX rounds_by_cell ON rounds (cell);
-- The interjections a matter holds the owner's words of, said in a work's group.
CREATE INDEX interjections_by_matter ON interjections (matter);
-- The interjections of a work delivered after a round: the words given since a merge.
CREATE INDEX interjections_by_work_delivered ON interjections (work, delivered_round);
-- The matters of a work of a kind: the questions an engineer asked the architect, the reports of its line.
CREATE INDEX architect_matters_by_work ON architect_matters (work, kind);
-- The matter an owner's ask put to the architect.
CREATE INDEX architect_matters_by_asked ON architect_matters (asked);
-- The notices about an owner's ask.
CREATE INDEX engine_notices_by_ask ON engine_notices (ask);
-- The blockers an owner's ask holds.
CREATE INDEX blockers_by_ask ON blockers (ask);
-- The blocker a matter of the architect's queue stands for.
CREATE INDEX blockers_by_matter ON blockers (matter);
-- The works on each flow version.
CREATE INDEX works_by_flow_version ON works (flow_version);

-- Views: what cells read and the owner's interface shows, computed from the tables. A view a cell reads for the item,
-- work or project at hand has a column of that name; the engine filters on it.

CREATE VIEW item_blocked_by AS
  -- Each item with each item it depends on by a blocking dependency that is not merged yet, as the merge cell
  -- (`M2`) records a merge: no work starts on the item until every one is. A cancelled item, which never
  -- delivers, holds back none. Read by: ready_items; the agents; the owner's interface, which
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
  -- graph does; it is done at the end of the engineer's graph, ended as `done` or, while its round there is not
  -- completed, not ended. While dispatch_holds has a row, the engine starts none.
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
    WHERE w.item IS NOT NULL AND (w.ended IS NULL OR w.ended = 'done') AND w.graph = 'engineer'
      AND w.at IN ('E1', '20', 'end'))
  SELECT i.id AS item, i.project, i.title, i.level, q.position, i.rank
  FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND i.held IS NULL AND i.id NOT IN (SELECT item FROM running)
    AND i.id NOT IN (SELECT item FROM item_blocked_by)
    AND (q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

CREATE VIEW open_items AS
  -- The items not done or cancelled, a short row each, in the order they start. An item's state: `running`, a work is
  -- on it, whose number and cell are in work and at; `paused`, that work is paused; `resuming`, it is paused and asked
  -- to go on, which waits for a free slot; `stopping`, a cancel or a start over was asked, and it reads as it stands
  -- until the work's step has ended; `held`, the architect holds it (items.held says why); `blocked`, an item it waits
  -- for is not merged (item_blocked_by); `ready`, it starts when a slot is free (ready_items). version is what a change
  -- to the item is written on. Read by: the agents, to see what is on hand.
  WITH
  running(item, work, at, paused) AS (
    SELECT w.item, w.id, w.at, w.paused FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.at NOT IN ('E1', '20', 'end') AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND (w.ended IS NULL OR w.ended = 'done') AND w.graph = 'engineer'
      AND w.at IN ('E1', '20', 'end'))
  SELECT i.id AS item, i.project, i.title,
    CASE
      WHEN EXISTS (
        SELECT 1 FROM outbox o
        WHERE (o.kind = 'start_over' AND o.work = r.work)
          OR (o.kind = 'cancel_item' AND json_extract(o.args, '$.item') = i.id)) THEN 'stopping'
      WHEN r.paused IS NOT NULL AND EXISTS (
        SELECT 1 FROM outbox o WHERE o.kind = 'resume_work' AND o.work = r.work) THEN 'resuming'
      WHEN r.paused IS NOT NULL THEN 'paused'
      WHEN r.item IS NOT NULL THEN 'running'
      WHEN i.held IS NOT NULL THEN 'held'
      WHEN i.id IN (SELECT item FROM item_blocked_by) THEN 'blocked'
      ELSE 'ready'
    END AS state,
    i.rank,
    (SELECT coalesce(max(c.id), 0) FROM changes c WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id))
      AS version,
    r.work, r.at
  FROM items i LEFT JOIN running r ON r.item = i.id LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND (r.item IS NOT NULL OR q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

CREATE VIEW item_body AS
  -- The item as its creator wrote it, the acceptance it wanted and the decision points it carries included. Read by:
  -- cells `R1`, `D1`, `I1`, `T2`, `T3` and `T6`.
  SELECT id AS item, project, title, problem, why, approach, deliver, wanted, decision_points FROM items;

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
  -- The item's acceptance, and the round that gave it, by its work and its number in the work; NULL until the work
  -- goes on to implement: until then what its creator wanted is `wanted` of item_body. locked says only whether there
  -- is one. Read by: cells `I1`, `T2`, `T3`, `T1`, `T6`, `V1` and
  -- `V2`.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;

CREATE VIEW item_verifications AS
  -- What each acceptance condition written in_effect came to, at each look of the cell that checks it once the work's
  -- change took effect (`V2`): the item, the work and the round, the condition's number (`id`), its verdict, and for a
  -- verdict that rests on a run the run and what it showed there. A condition stands as its latest round has it. Read
  -- by: the agents and the architect, by query.
  SELECT w.item, r.work, r.n AS round, json_extract(v.value, '$.id') AS id,
    json_extract(v.value, '$.verdict') AS verdict,
    json_extract(v.value, '$.run') AS run,
    json_extract(v.value, '$.seen') AS seen
  FROM rounds r JOIN works w ON w.id = r.work, json_each(r.output, '$.outcome.verdicts') v
  WHERE r.cell = 'V2' AND r.transition IS NOT NULL;

CREATE VIEW item_design_cards AS
  -- Every decision card of the item's works, by key, with its rulings, and why one withdrawn was. Read by: cells `R1`,
  -- `D1` and `D4`.
  SELECT w.item, c.work, c.id AS design_card, c.round, c.key, c.body, c.status, c.rulings, c.replaced_by, c.withdrawn
  FROM design_cards c JOIN works w ON w.id = c.work;

CREATE VIEW item_asks AS
  -- Every ask of the owner of the item's works, with its answer. Read by: cell `R1`.
  SELECT w.item, a.work, a.id AS ask, a.round, a.body, a.answer
  FROM owner_asks a JOIN works w ON w.id = a.work;

CREATE VIEW parent_design_cards AS
  -- The current decision cards of the items that depend on this one, open or ruled: what a split-off item's parent
  -- puts to the owner. Read by: cell `D4`.
  SELECT l.depends_on AS item, l.item AS parent, c.id AS design_card, c.key, c.body, c.status, c.rulings
  FROM item_links l
  JOIN works w ON w.item = l.item
  JOIN design_cards c ON c.work = w.id
  WHERE c.status IN ('open', 'ruled');

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
  -- their parent depends on it; below, how this item depends on it. Read by: the agents.
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

CREATE VIEW round_rows AS
  -- The row each round of a work took at its cell: the row its return took; for a round that ended in no return its
  -- cell takes, the shared row the engine took for it, which the round does not record as a transition, since a
  -- transition says the cell's own return was taken: the kind of the failed run (`<kind>_too_often` for the one past
  -- the kind's bound), `cut_off` for a round the engine cut off, `sent_back` for a return sent back. None for a round
  -- still open and for one that ended with the work paused. Read by: the engine, which counts how often a work took a
  -- row in a row, and which finds the counter of the shared row a round took when that round ends (`rounds.counts`).
  SELECT work, n AS round, cell,
    CASE
      WHEN transition IS NOT NULL THEN transition
      WHEN json_extract(rejected, '$.failed.over') IS NOT NULL THEN json_extract(rejected, '$.failed.kind') || '_too_often'
      WHEN json_extract(rejected, '$.failed.kind') IS NOT NULL THEN json_extract(rejected, '$.failed.kind')
      WHEN json_extract(rejected, '$.interrupted') IS NOT NULL THEN 'cut_off'
      WHEN json_extract(rejected, '$.error') IS NOT NULL THEN NULL
      WHEN rejected IS NOT NULL THEN 'sent_back'
    END AS took
  FROM rounds;

CREATE VIEW sent_back_problems AS
  -- Each problem of each return the engine sent back, one row each, in the order found (`position` counts from 0):
  -- the check that found it (`check_name`, which is `format` for a place that does not fit the cell's schema), the
  -- field and the text, as `rounds.rejected` keeps them under `problems`. A return sent back before `problems` was
  -- kept has no row. `field` is the check's `at` in the flow (`columns[pass].evidence`) for a check and the path in the
  -- schema with each list index written `[]` (`columns[].evidence`) for `format`, so group by `check_name` and
  -- `field` together. One return may hold several problems of one check and field: count returns with
  -- `count(DISTINCT work || '-' || round)` and problems with the rows. Read by: today only people and agents who query
  -- it, to count the returns sent back by check and by field. The architect's items on returns sent back (#198) and the
  -- merging of like ones (#257) will read it; no code reads it yet.
  SELECT r.work, r.n AS round, r.cell, p.key AS position,
    json_extract(p.value, '$.check') AS check_name,
    json_extract(p.value, '$.field') AS field,
    json_extract(p.value, '$.text') AS text
  FROM rounds r, json_each(r.rejected, '$.problems') p;

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

CREATE VIEW work_routes AS
  -- The way each work of the engineer went, from the rows it took: `turned`, a work that started to implement on the
  -- research's plan and then took the row counting `shallow_research`, which sent it to the design; `designed`, one
  -- that went to the design (cell `D1`), whether its research sent it there or not; `direct`, one whose research
  -- (cell `R1`) answered with its row `start`, so that it implements on the research's plan and has had no design; none
  -- before the research answers, and for a work of a flow version whose research took other rows. Nothing marks the
  -- item for a way: the research judges it. Read by: the owner's interface, beside the item's head; the agents.
  SELECT w.id AS work, w.item,
    CASE
      WHEN EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.counts = 'shallow_research') THEN 'turned'
      WHEN EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.cell = 'D1') THEN 'designed'
      WHEN (SELECT r.transition FROM rounds r
            WHERE r.work = w.id AND r.cell = 'R1' AND r.transition IN ('start', 'cannot_start')
            ORDER BY r.n DESC LIMIT 1) = 'start' THEN 'direct'
    END AS route
  FROM works w
  WHERE w.graph = 'engineer';

CREATE VIEW work_asking AS
  -- The cell that asked, or that took up an interjection, and how many times each work has asked: what choosing the
  -- way on needs of where the work stands. Read by: cell `A4`.
  SELECT w.id AS work, w.resume_at AS asked_at,
    (SELECT count(*) FROM rounds r WHERE r.work = w.id AND r.counts = 'asks') AS asks
  FROM works w;

CREATE VIEW work_design_cards AS
  -- Each work's decision cards as they stand: its current card of each key, open or ruled, with its rulings, the
  -- records it revises and the numbers of the earlier versions of its key it replaced. Read by: cells `T4` and `T5`.
  SELECT c.work, c.id AS design_card, c.round, c.key, c.body, c.revision, c.status, c.rulings,
    (SELECT json_group_array(o.id) FROM design_cards o WHERE o.work = c.work AND o.key = c.key AND o.id < c.id)
      AS replaces
  FROM design_cards c
  WHERE c.status IN ('open', 'ruled');

CREATE VIEW work_open_questions AS
  -- Each work's open questions as its design handed them in, each a matter of the project's architect, with the answer
  -- that came, which is the architect's reply: its own judgment, or the owner's answer it carried. Read by: cells `D1`,
  -- `D2` and `L2`; the engine, at cell `M1`.
  SELECT m.work, m.id AS matter, m.what AS question, json_extract(m.body, '$.answer') AS answer
  FROM architect_matters m
  WHERE m.kind = 'open_question';

CREATE VIEW work_owner_words AS
  -- What the owner said that concerns a work, one row for each record that holds it: the words said in the work's
  -- group to its project's architect (a matter of the kind owner_words), the owner's answer in the group to a question
  -- of its engineer's (the reply of a matter of the kind engineer_question), the words said to its engineer alone (an
  -- interjection no matter holds) and the owner's answers to the work's asks. A decision cites a row by its source and
  -- its id and quotes its words; the engine reads the words from the record again, and tells who said them from what
  -- the record kept. speaker is what the record kept of who was declared to speak, owner or delegate; none for a record
  -- kept before it was declared. relayed is, for words a delegate said, the parts of them it declared to be the owner's
  -- own words, a JSON array; none when it declared none. Read by: cell `L2`.
  SELECT json_extract(m.body, '$.work') AS work, 'matter' AS source, m.id AS id,
    json_extract(m.body, '$.speaker') AS speaker,
    coalesce(json_extract(m.body, '$.text'), json_extract(m.body, '$.answer')) AS words, m.at AS at,
    json_extract(m.body, '$.relayed') AS relayed
  FROM architect_matters m
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.where') = 'work'
  UNION ALL
  SELECT m.work, 'matter', m.id, json_extract(m.reply, '$.speaker'), json_extract(m.reply, '$.text'),
    coalesce(json_extract(m.reply, '$.at'), 0), json_extract(m.reply, '$.relayed')
  FROM architect_matters m
  WHERE m.kind = 'engineer_question' AND json_extract(m.reply, '$.by') = 'owner'
  UNION ALL
  SELECT j.work, 'interjection', j.id, j.declared, j.text,
    coalesce((SELECT max(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)
                AND c.old_row IS NULL), 0), j.relayed
  FROM interjections j
  WHERE j.speaker = 'owner' AND j.matter IS NULL AND j.cancelled_item IS NULL
  UNION ALL
  SELECT a.work, 'ask', a.id, json_extract(a.answer, '$.speaker'), json_extract(a.answer, '$.text'),
    coalesce(json_extract(a.answer, '$.at'), 0), json_extract(a.answer, '$.relayed')
  FROM owner_asks a
  WHERE json_extract(a.answer, '$.by') = 'owner';

CREATE VIEW work_ci AS
  -- The CI on each work: the latest run of the fast CI (the tool ci), of the full CI on the draft and of the tests on
  -- the code before the change (the tool ci_before), each with the head it ran on and its result, which holds the
  -- links to its runs; and the head last pushed. Read by: cells `T3` and `T4`.
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
  -- Interjections not yet handed in, in the order they came, each with the decision card it asks about, by its number
  -- and key, if it does. Read by: every agent cell; cells `T7`, `M1` and `E1`.
  SELECT j.work, j.id AS interjection, j.line, j.speaker, j.text, j.design_card, c.key AS card_key
  FROM interjections j LEFT JOIN design_cards c ON c.id = j.design_card
  WHERE j.delivered_round IS NULL;

CREATE VIEW merge_queue AS
  -- Each project's merge queue: the works at the merge cell, in the order they came to it; a work that left it and
  -- came back is last. Read by: cell `M2`.
  SELECT i.project, w.id AS work, w.pr, w.pushed_head
  FROM works w JOIN items i ON i.id = w.item
  WHERE w.at IN ('M2', '9', 'merge_queue', 'merge.queue') AND w.ended IS NULL
  -- The newest change of the row that changed `at` brought it to this cell. Scanned from the newest change back, so
  -- it stops after the few since then.
  ORDER BY (SELECT c.id FROM changes c
            WHERE c.table_name = 'works' AND c.row_key = json_object('id', w.id)
              AND json_extract(c.old_row, '$.at') IS NOT json_extract(c.new_row, '$.at')
            ORDER BY c.id DESC LIMIT 1), w.id;

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
  -- How often main was red only on known red checks, by project: as the architect's watch of main checked it, and as
  -- each engineer's work watched main after its merge before main was watched for the project. Read by: the owner's
  -- interface.
  SELECT coalesce(i.project, w.project) AS project, count(*) AS times
  FROM rounds r JOIN works w ON w.id = r.work LEFT JOIN items i ON i.id = w.item
  WHERE r.cell IN ('L1', '10', 'watch_main') AND json_extract(r.output, '$.main.kind') = 'known_red'
  GROUP BY coalesce(i.project, w.project);

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell on this very matter; the owner,
  -- for the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer and to which cell, the second cell a flow version before had a matter of two cells go on at, and whether
  -- its reporter marked it as harming now (urgent).
  -- Read by: cells `S0` and `N0`; the engine, which takes `S0`'s rows by it for each project whose architect runs no
  -- work, and takes the next matter by it in its fixed order; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w JOIN blockers b ON b.work = w.id
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL AND b.matter = m.id AND b.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AND m.goes_on_at IS NULL AS answered,
    m.resume_cell AS back_to,
    m.goes_on_at,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;

CREATE VIEW work_handled AS
  -- The matters each work of an architect handled, in the order it handled them: each matter, its kind, the cell and
  -- the round that closed it, and its record. Read by: the owner's interface; the engine, which gives a matter the log
  -- of what was handled on its item.
  SELECT m.handled_work AS work, m.handled_round AS round, m.id AS matter, m.kind, m.detail, m.what, r.cell,
    coalesce(json_extract(m.logged, '$.record'), json_extract(r.output, '$.progress')) AS said
  FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
  ORDER BY m.handled_work, m.handled_round;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the draft the architect wrote last (cell `P2`) before the round in which the
  -- owner's approval took effect (cell `P3`), in whichever of its works, with the round of that approval; none before
  -- the first policy takes effect. A flow version before set a policy in a setting of its own, whose text was written,
  -- or put together from a correction, last before the owner's latest approval in it, in whichever of the architect's
  -- works the setting went on in since its matter was raised; a setting a flow version before that held in a work of
  -- its own, or in one work of the architect, has the text in the work of the approval. Older versions stay readable in
  -- the rounds and are given to no agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      coalesce(
        (SELECT json_extract(t.output, '$.outcome.policy') FROM rounds t JOIN works y ON y.id = t.work
         WHERE y.graph = 'architect' AND y.project = w.project AND t.cell = 'P2' AND t.transition IS NOT NULL
           AND (t.work < a.work OR (t.work = a.work AND t.n < a.n))
           AND json_extract(t.output, '$.outcome.policy') IS NOT NULL
         ORDER BY t.work DESC, t.n DESC LIMIT 1),
        (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy'))
         FROM architect_matters m JOIN rounds t ON t.work BETWEEN m.work AND a.work JOIN works y ON y.id = t.work
         WHERE m.id = (SELECT max(x.id) FROM architect_matters x
                       WHERE x.project = w.project AND x.kind = 'set_policy'
                         AND (x.work < a.work OR (x.work = a.work AND x.round < a.n)))
           AND y.graph = 'architect' AND y.project = w.project AND t.cell IN ('P4', 'P5', 'P8')
           AND t.transition IS NOT NULL
           AND (t.work > m.work OR t.n > m.round) AND (t.work < a.work OR t.n < a.n)
           AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
             json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
         ORDER BY t.work DESC, t.n DESC LIMIT 1),
        (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy'))
         FROM rounds t
         WHERE t.work = a.work AND t.cell IN ('P4', 'write', 'policy.write', 'P5', 'assemble', 'policy.assemble', 'P8',
           'correct', 'policy.correct')
           AND t.transition IS NOT NULL AND t.n < a.n
           AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
             json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
         ORDER BY t.n DESC LIMIT 1)) AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY a.work DESC, a.n DESC) AS k
    FROM rounds a JOIN works w ON w.id = a.work
    WHERE w.graph IN ('architect', 'policy') AND a.cell IN ('P3', 'P12', 'takes_effect', 'policy.takes_effect')
      AND a.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW policy_draft AS
  -- Each project's draft of a new policy that waits for the owner's approval: the latest the architect wrote (cell
  -- `P2`), in whichever of its works, when no new policy took effect after it (cell `P3`) and no later round of `P2`
  -- dropped it, the owner's reply keeping the policy in force (`kept`, with no policy); none otherwise. Read by: cell
  -- `P2`, which writes it again by the owner's reply, and cell `P3`, which makes it the policy in force; the owner's
  -- interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.policy') AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell = 'P2' AND r.transition IS NOT NULL) d
  WHERE k = 1 AND policy IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM rounds a JOIN works x ON x.id = a.work
    WHERE x.graph = 'architect' AND x.project = d.project AND a.cell IN ('P3', 'P12') AND a.transition IS NOT NULL
      AND (a.work > d.work OR (a.work = d.work AND a.n > d.round)));

CREATE VIEW notebook AS
  -- Each project's architect's notebook as it stands: each part's latest version, with the round that wrote it.
  -- A new base writes its understanding from nothing, and the next plan is given the plan before.
  -- Hidden from agents.
  -- Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, section, text, work, round FROM (
    SELECT project, section, text, work, round,
      row_number() OVER (PARTITION BY project, section ORDER BY id DESC) AS k
    FROM notebook_edits)
  WHERE k = 1;

CREATE VIEW observations_all AS
  -- Every observation of each project as the owner's page shows it: closed or not and why (one an item took in is
  -- closed once that item's work merged), with when it was recorded and last changed, as changes has it.
  -- Read by: the owner's interface; observations_open; the engine, for the overview the architect is given.
  SELECT o.project, o.id, o.title, o.what, o.tags, o.times_reported, o.source, o.refs, o.folded_into,
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
  -- Each project's open observations, each with its number, its title, what it is, its tags, how many times it was
  -- reported, where it came from, what it refers to and the item that took it in, if one has. Read by: the agents;
  -- the engine.
  SELECT project, id, title, what, tags, times_reported, source, refs, folded_into
  FROM observations_all WHERE closed IS NULL;

CREATE VIEW conversation_entries AS
  -- Every entry of the owner's conversations, gathered from the tables that hold them: a group for each engineer's
  -- work, whose members are the owner, its engineer and its project's architect (what the engineer and the architect
  -- tell and ask the owner and each other about the work, the architect's messages while it handles a matter of the
  -- work, the owner's words to either or both and the architect's reply to its own, the design cards and the rulings
  -- on them, and the engine's notices about it); the owner's one-to-one with each project's architect (the owner's
  -- words to it and its replies, those a flow version before took while a talk with it was open being entries of that
  -- talk, what it tells and asks the owner about no work, in its works and in the settings of its policy a flow
  -- version before ran); one for the architects of projects the configuration connects (what they say
  -- to each other, and the owner's words to them); and one of wf-agents itself (its faults, and what the engine tells
  -- the owner about no work). The owner's words said to the architect's work in hand or to its policy setting while it
  -- runs, and its answers to them, are in the one-to-one, whatever matter it handles. A work's helpers (its advisor,
  -- review and adversarial testing) are in no group: what they and the engineer say to each other is in the work's
  -- rounds.
  -- Hidden from agents.
  -- Read by: the owner's interface, which lists the conversations, counts what waits and what is unread in each, and
  -- shows each.
  -- conversation is work:<id>, architect:<project>, architects:<project>[|<project>] (the projects' architects; the
  -- interface takes it to the group of every project the configuration connects them with), or wf-agents. source is
  -- what the entry is (message, notice, ask, interjection, words, talk, question, report, reply, card, ruling or
  -- between) and id its row in the table it comes from; an ask is one entry, the owner's answer to it shown in it, never
  -- an entry of its own beside it. sender and recipient are a line (the architect for its base and for a matter's
  -- session forked from it), the owner, the engine or wf-agents, the recipient of
  -- the owner's words in a work's group the engineer, the architect or both, and in the one-to-one the line of the
  -- architect's work they were said to (architect or policy); for between, the projects. matter: the architect's
  -- matter the entry is, or holds the owner's words of, or whose talk it is an entry of. design_card: the decision
  -- card the entry is, or is a ruling on, or the owner's question asks about, or an answer answers about. waiting: it waits for the
  -- owner (an ask that stands for an open blocker the owner holds, an architect's ask that holds a matter still
  -- open, neither handled nor withdrawn, or an ask the work goes on without while it runs, not answered; a design card not ruled while its work goes on; a fault not
  -- read). pending: the owner's words not taken
  -- up yet. undelivered: the owner's words to an agent that its work ended without taking in. answerable: an
  -- engineer's question the architect holds now, which the owner may answer in the group instead.
  SELECT CASE WHEN w.graph = 'engineer' THEN 'work:' || w.id
              WHEN r.transition = 'answers' THEN 'architect:' || w.project
              ELSE coalesce((SELECT CASE WHEN y.graph = 'engineer' THEN 'work:' || x.work
                                         WHEN x.kind = 'owner_words' AND json_extract(x.body, '$.where') = 'work'
                                           THEN 'work:' || json_extract(x.body, '$.work') END
                             FROM architect_matters x LEFT JOIN works y ON y.id = x.work
                             WHERE x.id = (
                               SELECT coalesce(json_extract(p.output, '$.next.matter'),
                                 json_extract(p.output, '$.outcome.pick.matter'),
                                 json_extract(p.output, '$.queue.matter'))
                               FROM rounds p
                               WHERE p.work = m.work AND p.n < m.round AND p.transition IS NOT NULL
                                 AND coalesce(json_extract(p.output, '$.next.matter'),
                                   json_extract(p.output, '$.outcome.pick.matter'),
                                   json_extract(p.output, '$.queue.matter')) IS NOT NULL
                               ORDER BY p.n DESC LIMIT 1)
                               AND (x.handled_round IS NULL OR x.handled_work <> m.work OR x.handled_round >= m.round)),
                            'architect:' || w.project) END AS conversation,
    coalesce(w.project, i.project) AS project, 'message' AS source, m.id AS id,
    CASE WHEN w.graph NOT IN ('architect', 'policy') THEN coalesce(s.line, 'engineer')
         WHEN s.line IS NULL OR s.line IN ('architect', 'matter') THEN 'architect' ELSE s.line END AS sender,
    'owner' AS recipient, m.text AS text, coalesce(r.ended_at, r.started_at) AS at, m.work AS work, m.round AS round,
    NULL AS ask, m.design_card AS design_card, NULL AS matter, m.read_at AS read_at, 0 AS waiting, 0 AS pending,
    0 AS undelivered, 0 AS answerable, NULL AS fault
  FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
  LEFT JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
  UNION ALL
  SELECT CASE WHEN n.fault IS NOT NULL OR w.id IS NULL THEN 'wf-agents'
              WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    CASE WHEN n.fault IS NULL THEN coalesce(w.project, i.project) END, 'notice', n.id,
    CASE WHEN n.fault IS NULL THEN 'engine' ELSE 'wf-agents' END, 'owner', n.text, n.at, n.work, NULL, n.ask, NULL,
    NULL, n.read_at, n.fault IS NOT NULL AND n.read_at IS NULL, 0, 0, 0, n.fault
  FROM engine_notices n LEFT JOIN works w ON w.id = n.work LEFT JOIN items i ON i.id = w.item
  WHERE n.ask IS NULL OR n.fault IS NOT NULL
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'ask', a.id,
    CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect'
         ELSE coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
                        WHERE x.work = a.work AND x.n < a.round ORDER BY x.n DESC LIMIT 1), 'engineer') END,
    'owner',
    coalesce(json_extract(a.body, '$.question'),
             (SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(a.body, '$.asked') q, json_each(q.value, '$.value.points') p),
             (SELECT n.text FROM engine_notices n WHERE n.ask = a.id ORDER BY n.id LIMIT 1), ''),
    coalesce((SELECT min(n.at) FROM engine_notices n WHERE n.ask = a.id), r.started_at), a.work, a.round, a.id, NULL,
    NULL, NULL,
    a.answer IS NULL AND (
      (w.graph = 'architect' AND EXISTS (
        SELECT 1 FROM architect_matters m WHERE m.asked = a.id AND m.handled_round IS NULL AND m.withdrawn IS NULL))
      OR EXISTS (SELECT 1 FROM blockers b WHERE b.ask = a.id AND b.ended IS NULL AND b.holder = 'owner')
      OR (w.ended IS NULL AND json_extract(a.body, '$.kind') IN ('design_review', 'points', 'change'))), 0, 0, 0, NULL
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'interjection', j.id, j.speaker,
    CASE WHEN j.matter IS NULL THEN j.line WHEN j.informs = 1 THEN 'architect' ELSE 'both' END,
    coalesce((SELECT json_extract(x.body, '$.text') FROM architect_matters x WHERE x.id = j.matter), j.text),
    coalesce((SELECT max(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)
                AND c.old_row IS NULL),
             (SELECT min(x.started_at) FROM rounds x WHERE x.work = j.work), 0),
    j.work, j.delivered_round, NULL, j.design_card, j.matter, NULL, 0,
    j.speaker = 'owner' AND ((j.informs = 0 AND j.delivered_round IS NULL AND w.ended IS NULL
        AND w.at NOT IN (CASE WHEN w.graph = 'architect' THEN 'R1' ELSE 'E1' END, 'E2', '20', 'end'))
      OR EXISTS (SELECT 1 FROM architect_matters x
                 WHERE x.id = j.matter AND x.handled_round IS NULL AND x.withdrawn IS NULL)),
    j.speaker = 'owner' AND j.informs = 0 AND j.delivered_round IS NULL
      AND (w.ended IS NOT NULL OR w.at IN (CASE WHEN w.graph = 'architect' THEN 'R1' ELSE 'E1' END, 'E2', '20', 'end')),
    0, NULL
  FROM interjections j JOIN works w ON w.id = j.work LEFT JOIN items i ON i.id = w.item
  WHERE j.text NOT LIKE 'Ruling on design card %'
    AND j.text NOT LIKE 'The owner confirmed every decision of design card %'
    AND j.text NOT LIKE 'Ruling on decision card %'
    AND j.text NOT LIKE 'The owner confirmed decision card %'
  UNION ALL
  SELECT CASE WHEN json_extract(m.body, '$.where') = 'projects' THEN 'architects:' || m.project
              WHEN y.graph = 'engineer' THEN 'work:' || y.id ELSE 'architect:' || m.project END,
    m.project, 'words', m.id, 'owner',
    CASE WHEN json_array_length(m.body, '$.to') > 1 THEN 'architects' ELSE 'architect' END,
    json_extract(m.body, '$.text'), m.at, y.id, NULL, NULL, NULL, m.id, NULL, 0,
    m.handled_round IS NULL AND m.withdrawn IS NULL, 0, 0, NULL
  FROM architect_matters m LEFT JOIN works y ON y.id = json_extract(m.body, '$.work')
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.text') IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM interjections j WHERE j.matter = m.id)
    AND (json_extract(m.body, '$.where') IS NOT 'projects' OR json_extract(m.body, '$.to[0]') = m.project)
  UNION ALL
  SELECT 'architect:' || m.project, m.project, 'talk', e.id, 'owner', 'architect', e.text, e.at, NULL, NULL, NULL,
    NULL, e.matter, NULL, 0, e.taken_round IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL, 0, 0, NULL
  FROM talk_entries e JOIN architect_matters m ON m.id = e.matter
  UNION ALL
  SELECT 'work:' || m.work, m.project, CASE WHEN m.kind = 'report' THEN 'report' ELSE 'question' END, m.id,
    coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
              WHERE x.work = m.work AND x.n <= m.round ORDER BY x.n DESC LIMIT 1), 'engineer'),
    'architect',
    coalesce((SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(m.body, '$.asked') q, json_each(q.value, '$.value.points') p), m.what),
    m.at, m.work, m.round, NULL, NULL, m.id, NULL, 0, 0, 0,
    m.reply IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL AND EXISTS (
      SELECT 1 FROM blockers b WHERE b.matter = m.id AND b.ended IS NULL AND b.holder = 'architect'),
    NULL
  FROM architect_matters m JOIN works w ON w.id = m.work
  WHERE w.graph = 'engineer' AND m.kind IN ('engineer_question', 'failure', 'report')
  UNION ALL
  SELECT 'work:' || m.work, m.project, 'reply', m.id,
    CASE WHEN json_extract(m.reply, '$.by') = 'owner' THEN 'owner' ELSE 'architect' END,
    CASE WHEN json_extract(m.reply, '$.hand_over.to') = 'owner' THEN 'owner'
         ELSE coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
                        WHERE x.work = m.work AND x.n <= m.round ORDER BY x.n DESC LIMIT 1), 'engineer') END,
    coalesce(json_extract(m.reply, '$.text'), ''),
    coalesce(json_extract(m.reply, '$.at'),
             (SELECT x.ended_at FROM rounds x WHERE x.work = m.handled_work AND x.n = m.handled_round), m.at),
    m.work, m.round, NULL, NULL, m.id, NULL, 0, 0, 0, 0, NULL
  FROM architect_matters m JOIN works w ON w.id = m.work
  WHERE w.graph = 'engineer' AND m.reply IS NOT NULL
  UNION ALL
  SELECT 'work:' || c.work, i.project, 'card', c.id, coalesce(s.line, 'engineer'), 'owner', '',
    coalesce(cr.ended_at, cr.started_at), c.work, c.round, NULL, c.id, NULL, NULL,
    c.status = 'open' AND w.ended IS NULL, 0, 0, 0, NULL
  FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
  JOIN rounds cr ON cr.work = c.work AND cr.n = c.round LEFT JOIN sessions s ON s.id = cr.session
  WHERE c.status IN ('open', 'ruled')
  UNION ALL
  SELECT 'work:' || c.work, i.project, 'ruling', c.id, 'owner', 'engineer', '',
    coalesce(json_extract(x.value, '$.at'), cr.ended_at, cr.started_at), c.work, c.round, NULL, c.id, NULL, NULL, 0,
    0, 0, 0, NULL
  FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
  JOIN rounds cr ON cr.work = c.work AND cr.n = c.round, json_each(c.rulings) x
  WHERE c.status IN ('open', 'ruled') AND json_extract(x.value, '$.by') = 'owner'
  UNION ALL
  SELECT 'architects:' || min(json_extract(m.body, '$.from'), m.project) || '|'
      || max(json_extract(m.body, '$.from'), m.project),
    NULL, 'between', m.id, json_extract(m.body, '$.from'), m.project, coalesce(json_extract(m.body, '$.text'), m.what),
    m.at, json_extract(m.body, '$.work'), NULL, NULL, NULL, m.id, NULL, 0, 0, 0, 0, NULL
  FROM architect_matters m
  WHERE m.kind = 'other_architect' AND json_extract(m.body, '$.from') IS NOT NULL;

CREATE VIEW conversation_shown AS
  -- The entries of the owner's conversations that the owner's page shows and the architect is told of. They are
  -- those of conversation_entries but the engine's notices in a work's group and in an architect's one-to-one, which
  -- the bubble of the stage the work was at, the bubble of the matter the architect has in hand and their heads say
  -- instead; the side reports a work's lines handed in, in a work's group, which are the architect's to read in its
  -- queue; and what a helper of the work (the review, the adversarial testing, the advisor) or of the architect (the
  -- review of its batch, the search of its observations) told the owner, whose words go to the one it helps: a work's
  -- round's progress note in the stage's bubble says it, and the architect reads the rest. This is the one
  -- definition, so that what the page shows of a conversation and what the architect is told of its one-to-one are the
  -- same entries.
  -- Hidden from agents.
  -- Read by: the owner's interface, which lists the conversations and shows each; the engine, which gives a new session
  -- of an architect what was said in its one-to-one since its previous session began.
  SELECT * FROM conversation_entries
  WHERE NOT ((conversation LIKE 'work:%' OR conversation LIKE 'architect:%') AND source = 'notice')
    AND NOT (conversation LIKE 'work:%' AND source = 'report')
    AND NOT ((conversation LIKE 'work:%' OR conversation LIKE 'architect:%') AND source = 'message'
      AND sender NOT IN ('engineer', 'architect', 'policy'));
