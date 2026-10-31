-- Takes a version 10 database to version 11. A round is known by its work and its number in the work, counted from 1,
-- and named R<work>-<n>, instead of by one number across all works: the rounds of each work are numbered in the order
-- of their old numbers, and every row that pointed at a round by its old number points at it by its work and its
-- number. What was sent and kept stays as it was: the rounds' records and the history in changes. Each statement below
-- that defines something is written exactly as db/schema.sql writes it.

-- The views that read a round by its old number go first, and come back reading it by its work and number.
DROP VIEW ready_items;
DROP VIEW item_problem;
DROP VIEW item_acceptance;
DROP VIEW item_design_cards;
DROP VIEW item_asks;
DROP VIEW parent_design_cards;
DROP VIEW work_history;
DROP VIEW answered_points;
DROP VIEW work_measurements;
DROP VIEW work_review_record;
DROP VIEW waited_items;
DROP VIEW side_reports;
DROP VIEW work_followups;
DROP VIEW work_progress;
DROP VIEW item_research;
DROP VIEW adversarial_directions;
DROP VIEW advisor_rulings;
DROP VIEW not_applicable_answers;

-- Each round's number in its work, by the order of the old numbers.
CREATE TEMP TABLE round_numbers AS
  SELECT id, work, row_number() OVER (PARTITION BY work ORDER BY id) AS n FROM rounds;

-- The rounds, and each table that holds a round's old number, are made afresh with their rows; every row but a round
-- keeps its id. A migration runs with the foreign keys off, as SQLite makes a table afresh, and every one is checked
-- before the migration commits.
CREATE TABLE items_version_10 AS SELECT * FROM items;
CREATE TABLE rounds_version_10 AS SELECT * FROM rounds;
CREATE TABLE design_cards_version_10 AS SELECT * FROM design_cards;
CREATE TABLE owner_asks_version_10 AS SELECT * FROM owner_asks;
CREATE TABLE owner_messages_version_10 AS SELECT * FROM owner_messages;
CREATE TABLE tool_runs_version_10 AS SELECT * FROM tool_runs;
CREATE TABLE interjections_version_10 AS SELECT * FROM interjections;
DROP TABLE items;
DROP TABLE rounds;
DROP TABLE design_cards;
DROP TABLE owner_asks;
DROP TABLE owner_messages;
DROP TABLE tool_runs;
DROP TABLE interjections;

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
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO items (id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work,
    acceptance_round, cancelled, imported_from, imported_branch, imported_pr)
  SELECT i.id, i.project, i.title, i.problem, i.why, i.approach, i.deliver, i.level, i.acceptance, n.work, n.n,
    i.cancelled, i.imported_from, i.imported_branch, i.imported_pr
  FROM items_version_10 i LEFT JOIN round_numbers n ON n.id = i.acceptance_round
  ORDER BY i.id;

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

INSERT INTO rounds (work, n, cell, session, input, output, transition, rejected, config_commit, model, usage,
    compactions, started_at, ended_at)
  SELECT r.work, n.n, r.cell, r.session, r.input, r.output, r.transition, r.rejected, r.config_commit, r.model, r.usage,
    r.compactions, r.started_at, r.ended_at
  FROM rounds_version_10 r JOIN round_numbers n ON n.id = r.id
  ORDER BY r.id;

CREATE TABLE design_cards (
  -- A change to the design tree for the owner to confirm before it is merged: each decision the change adds, changes
  -- or removes. It asks the owner nothing; what the owner is asked is an ask of the owner. A design card is handed in
  -- by one round and belongs to that round; its item is found through the round's work. A ruled design card never
  -- changes; a decision that has to change comes in a new design card that replaces the old one.
  -- Read by: cell 1 (design cards ruled on this item), 2 and 2a (rulings that disagreed or chose another option), 2a
  -- (the latest design card of the items that depend on this one), 15 (the design card to look at), 19 (whether the
  -- design card has been confirmed).
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
  -- The rulings in the order given, JSON list: who ruled (owner or architect), the verdict, the option chosen and
  -- their own words. Read by: cells 1, 2, 2a and 19.
  rulings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(rulings)),
  -- The design card that replaced this one. Read by: people following a decision to its latest design card.
  replaced_by INTEGER REFERENCES design_cards(id),
  -- A replaced design card names its successor, unless its work ended without one.
  CHECK (replaced_by IS NULL OR status = 'replaced'),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO design_cards (id, work, round, body, status, rulings, replaced_by)
  SELECT c.id, n.work, n.n, c.body, c.status, c.rulings, c.replaced_by
  FROM design_cards_version_10 c JOIN round_numbers n ON n.id = c.round
  ORDER BY c.id;

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

INSERT INTO owner_asks (id, work, round, body, answer)
  SELECT a.id, n.work, n.n, a.body, a.answer
  FROM owner_asks_version_10 a JOIN round_numbers n ON n.id = a.round
  ORDER BY a.id;

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

INSERT INTO owner_messages (id, work, round, text, read_at)
  SELECT m.id, n.work, n.n, m.text, m.read_at
  FROM owner_messages_version_10 m JOIN round_numbers n ON n.id = m.round
  ORDER BY m.id;

CREATE TABLE tool_runs (
  -- Each attempt of a tool the engine runs: the tools an agent asks for, run at cell 13, and the tool cells 7, 9 and
  -- 10. A retry is a new row.
  -- Read by: the round that asked for the tools (their results); cells 5 and 6 (each measurement); 15 (a tool that
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

INSERT INTO tool_runs (id, work, round, tool, args, attempt, result, files, queued_at, started_at, ended_at)
  SELECT t.id, n.work, n.n, t.tool, t.args, t.attempt, t.result, t.files, t.queued_at, t.started_at, t.ended_at
  FROM tool_runs_version_10 t JOIN round_numbers n ON n.id = t.round
  ORDER BY t.id;

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

INSERT INTO interjections (id, work, line, speaker, text, delivered_round)
  SELECT x.id, x.work, x.line, x.speaker, x.text, n.n
  FROM interjections_version_10 x LEFT JOIN round_numbers n ON n.id = x.delivered_round
  ORDER BY x.id;

DROP TABLE items_version_10;
DROP TABLE rounds_version_10;
DROP TABLE design_cards_version_10;
DROP TABLE owner_asks_version_10;
DROP TABLE owner_messages_version_10;
DROP TABLE tool_runs_version_10;
DROP TABLE interjections_version_10;
DROP TABLE round_numbers;

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

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: queued, not cancelled, and with no work running. An item leaves
  -- the queue when a work starts on it, and may be queued again once its works ended. A work runs until it ends or
  -- its last round, at the cell it is at, completed without taking a row, which only the end of its graph does.
  -- Read by: cell 0.
  SELECT i.id AS item, i.project, i.title, i.level, q.position
  FROM dispatch_queue q JOIN items i ON i.id = q.item
  WHERE i.cancelled IS NULL AND NOT EXISTS (
    SELECT 1 FROM works w
    WHERE w.item = i.id AND w.ended IS NULL AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL))
  ORDER BY q.position;

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
  -- Every design card of the item's works, with its rulings. Read by: cells 1, 2, 2a, 15 and 19.
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

CREATE VIEW work_history AS
  -- Every round of each work in order: the cell, what the engine gave, the return, the row taken or why the
  -- return was sent back, and the tools it ran. Read by: the owner's history command.
  SELECT r.work, r.n AS round, r.cell, r.input, r.output, r.transition, r.rejected, r.started_at, r.ended_at,
    (SELECT json_group_array(json_object('tool', t.tool, 'attempt', t.attempt, 'result', json(t.result)))
     FROM tool_runs t WHERE t.work = r.work AND t.round = r.n) AS tools
  FROM rounds r;

CREATE VIEW answered_points AS
  -- The points the advisor answered for each work, by round. Read by: cells 11 and 12.
  SELECT r.work, r.n AS round, p.value AS point
  FROM rounds r, json_each(r.output, '$.outcome.points') p
  WHERE r.cell = '12' AND r.transition IS NOT NULL;

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

CREATE VIEW side_reports AS
  -- Every side report a round handed in. Read by: cell 15; the owner's interface.
  SELECT r.work, r.n AS round, r.cell, s.value AS report
  FROM rounds r, json_each(r.output, '$.side_reports') s
  WHERE r.output IS NOT NULL;

CREATE VIEW work_followups AS
  -- What to do next, as each report or fix handed in. Read by: cell 20; the owner's interface.
  SELECT w.item, r.work, r.n AS round, json_extract(r.output, '$.outcome.end.followup') AS followup
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE r.cell IN ('4', '8') AND json_extract(r.output, '$.outcome.end.followup') IS NOT NULL;

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
