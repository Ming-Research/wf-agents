-- Takes a version 44 database to version 45: the setting of a project's policy is a stage of its architect's works,
-- in the architect's session, instead of a work of its own of the graph policy. A setting is a matter of the kind
-- set_policy, which may go on over several works of the architect, and the views of the policy being set read its
-- rounds by that matter; the view of the policy in force reads the text of a setting in whichever work it was written;
-- the round that puts a study before the owner, and holds the setting until the answer comes, is no entry of the
-- architect's log. A matter that said a setting of a work of its own had ended, whose path was what to do next, is
-- withdrawn: no such matter comes any longer, and the architect's next understanding thinks what to do next. A setting
-- still running as a work of its own is taken over by the engine when it next goes on, as every work on an older flow
-- version is: it ends, its hold on new starts is let go, and the architect's next check of the policy decides again.
-- The descriptions of the works, the sessions, the architects' matters and the blockers say so, so those tables are
-- made afresh with their rows, the architects' matters with an index of the settings. Each statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW policy_readings;
DROP VIEW policy_trials;
DROP VIEW policy_probes_asked;
DROP VIEW policy_text;
DROP VIEW policy_draft;
DROP VIEW policy_current;
DROP VIEW architect_log;

UPDATE architect_matters
SET withdrawn = 'The setting of a policy that ended is a stage of the architect''s works now, which goes on to what to do next itself'
WHERE kind = 'policy' AND handled_round IS NULL AND withdrawn IS NULL;

CREATE TABLE works_version_44 AS SELECT * FROM works;
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
  flow_version INTEGER NOT NULL REFERENCES flow_versions(id),
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
FROM works_version_44;
DROP TABLE works_version_44;
CREATE INDEX works_by_item ON works (item);

CREATE TABLE sessions_version_44 AS SELECT * FROM sessions;
DROP TABLE sessions;

CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session's context is past the configuration's limit, when the model its rounds run on
  -- changed, whose prompt cache it would have to write again whole, and when the owner asks for one. Each project's
  -- architect's line goes on in one CLI session across the works of the project's architect, the setting of a new
  -- policy included: each has a row of its own with the same cli_session. A cell on no line has a session of its own
  -- for each round, ended once the round has.
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
  -- architect work left;
  -- owner, the owner asked for a new session with the round's Start or Step on the page. Read by: the owner's
  -- interface.
  started TEXT NOT NULL DEFAULT 'line' CHECK (started IN ('line', 'cell', 'limit', 'model', 'carried', 'owner'))
) STRICT;

INSERT INTO sessions (id, work, line, cli, cli_session, ended, started)
SELECT id, work, line, cli, cli_session, ended, started
FROM sessions_version_44;
DROP TABLE sessions_version_44;
CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;

CREATE TABLE architect_matters_version_44 AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, or a place its understanding found to set straight. The project's queue is its
  -- matters not handled; its architect picks among them by the constitution and the policy, not by the order they
  -- came. A matter in the queue starts a work of the project's architect when none runs, but for flow feedback, the
  -- project's own or forwarded by another project's architect, which waits for a work something else started and is
  -- picked only when it came before that work started.
  -- Read by: the engine, which starts the architect's works, picks a lone matter, routes each to its path, hands it its
  -- material and takes its replies back; the architect's cells through architect_queue and work_handled; the owner's
  -- interface.
  id INTEGER PRIMARY KEY,
  -- The project whose architect it is for.
  project TEXT NOT NULL,
  -- Its kind, which decides the path it takes: set_straight, engineer_question, report, design_card, failure,
  -- owner_words, other_architect, slots_free, or set_policy for the setting of a new policy, which the check of the
  -- policy in force raises and takes in hand at once, and which may go on over several works of the architect. A
  -- matter handled before these kinds keeps the kind it had then, as policy did for a setting that ended, when the
  -- setting was a work of its own.
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
  -- the commit merged, the design card, the tool that failed, the owner's words, the message, the place to set
  -- straight; for a fault forwarded, who forwarded it (by: owner, or setting for the owner's setting), the fault
  -- itself being read from its notice when the matter is handled. Read by: the engine, for the matter's material.
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
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the owner's
  -- reading of a study or approval of a policy, the cell that put it, whose rows go on by the owner's choice.
  resume_cell TEXT,
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. The owner's answer in the
  -- work's group to a question the architect holds is the reply too, said by the owner (by). Read by: the engine,
  -- which takes it back unchanged to the work that asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the item changes of its round to be made, JSON, as the round handed it in: it becomes the
  -- reply, with the numbers of the items they made, once the review of their batch lets them be made. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its work and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; the owner answered the question in the work's group, the reply saying what; or it said that a
  -- setting of the policy, a work of its own then, had ended, when the setting became a stage of the architect's
  -- works. NULL otherwise.
  -- Read by: architect_queue, which leaves it out; the engine, which tells a new work on the item what an earlier
  -- work asked and had withdrawn; the owner's interface, which leaves it out of the queue.
  withdrawn TEXT,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO architect_matters (id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, reply, reply_held, handled_work, handled_round, withdrawn)
SELECT id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, reply, reply_held, handled_work, handled_round, withdrawn
FROM architect_matters_version_44;
DROP TABLE architect_matters_version_44;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE INDEX architect_matters_settings ON architect_matters (project, id) WHERE kind = 'set_policy';
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;

CREATE TABLE blockers_version_44 AS SELECT * FROM blockers;
DROP TABLE blockers;

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

INSERT INTO blockers (id, work, cell, round, kind, what, holder, project, items, matter, ask, at, since, moves, said, ended, outcome, ended_at)
SELECT id, work, cell, round, kind, what, holder, project, items, matter, ask, at, since, moves, said, ended, outcome, ended_at
FROM blockers_version_44;
DROP TABLE blockers_version_44;
CREATE UNIQUE INDEX blockers_open ON blockers (work) WHERE ended IS NULL;

CREATE VIEW architect_log AS
  -- Each project's architect's log, newest first, its latest forty entries: each judgment it handed in (whether the
  -- policy is right, whether what runs fits, each matter's answer, each rethink after a batch) and the owner's answer
  -- to each study of a new policy, by the round that gave it, with the matter it handled and what it said, its
  -- conclusion with its reasons; the round that put the study before the owner, and holds the setting until the answer
  -- comes, is no entry. Read by: cells `U1`, `P1` and `N1`.
  SELECT project, work, round, cell, at, matter, said FROM (
    SELECT w.project, r.work, r.n AS round, r.cell, r.ended_at AS at,
      (SELECT m.id FROM architect_matters m WHERE m.handled_work = r.work AND m.handled_round = r.n) AS matter,
      coalesce(json_extract(r.output, '$.progress'),
        json_extract(r.output, '$.ruling.choice') || ': ' || json_extract(r.output, '$.ruling.text')) AS said,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph IN ('architect', 'policy') AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers', 'held')
      AND r.cell IN ('P1', 'U2', 'policy_right', 'understand.policy', 'N1', 'U3', 'fit', 'understand.fit', 'H1', 'T1',
        'set_straight', 'handle.set_straight', 'H3', 'ways', 'handle.ways', 'H5', 'I2', 'take_in', 'handle.take_in',
        'H6', 'K1', 'card_sound', 'handle.card_sound', 'H7', 'F1', 'have_fixed', 'handle.have_fixed', 'H8', 'O1',
        'owner_words', 'handle.owner_words', 'H9', 'M1', 'answer_architect', 'handle.answer_architect', 'H10',
        'what_runs', 'handle.what_runs', 'B4', 'C1', 'rethink', 'batch.rethink', 'P3', 'owner_reads',
        'policy.owner_reads'))
  WHERE k <= 40;

CREATE VIEW policy_setting_rounds AS
  -- Each round of a setting of a project's policy at a cell of its stage, with the setting's matter, of the kind
  -- set_policy. A setting is a stage of the project's architect's works, which go on one after another: the check of
  -- the policy in force that sets it again raises its matter, and its rounds are those at the cells of the stage after
  -- that, in any of the works it goes on in, until the next setting's matter. A setting a flow version before held in
  -- a work of its own, or in one work of the architect, is not here. Read by: policy_draft and policy_trials.
  SELECT project, work, n, cell, transition, output, matter FROM (
    SELECT w.project, r.work, r.n, r.cell, r.transition, r.output,
      (SELECT max(m.id) FROM architect_matters m
       WHERE m.project = w.project AND m.kind = 'set_policy'
         AND (m.work < r.work OR (m.work = r.work AND m.round < r.n))) AS matter
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('P2', 'P3', 'P4', 'P5', 'P6', 'P7', 'P8', 'P9', 'P10', 'P11', 'P12'))
  WHERE matter IS NOT NULL;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written, or put together from a correction, last before the owner's latest
  -- approval in a setting of the project's policy, in whichever of its architect's works the setting went on in since
  -- its matter was raised, with the round of that approval; none before the first policy takes effect. A setting a flow
  -- version before held in a work of its own, or in one work of the architect, has the text in the work of the
  -- approval. A correction hands in only what it changes, and the engine puts the text together (`P5`); a correction
  -- of a flow version before that held the whole text. Older versions stay readable in the rounds and are given to no
  -- agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      coalesce(
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
    WHERE w.graph IN ('architect', 'policy') AND a.cell IN ('P12', 'takes_effect', 'policy.takes_effect')
      AND a.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW policy_draft AS
  -- Each setting's version of the policy being set, by the setting's matter: the round of the latest writing or
  -- correcting in it, its work and number, the text as it stands, and the probes with the answers the architect holds
  -- right. A correction hands in only what it changes, its edits and the probes it adds or changes beside those it
  -- keeps, and the text and the probes are then as the engine last put them together (`P5`), as the text is after a
  -- minor fix a reading found, which leaves the version as it was. A round of those cells that asked for tools, asked
  -- the owner, paused, answered the owner's words or gave way to a new session wrote no version. Read by: policy_text,
  -- policy_probes_asked and policy_trials; the cells on the architect's line take it from the returns that wrote it or
  -- put it together.
  SELECT t.matter, t.work, t.n AS round,
    (SELECT coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
       json_extract(x.output, '$.outcome.revision.policy'))
     FROM policy_setting_rounds x
     WHERE x.matter = t.matter AND x.cell IN ('P4', 'P5', 'P8') AND x.transition IS NOT NULL
       AND coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
         json_extract(x.output, '$.outcome.revision.policy')) IS NOT NULL
     ORDER BY x.work DESC, x.n DESC LIMIT 1) AS policy,
    coalesce(
      (SELECT json_extract(x.output, '$.draft.probes') FROM policy_setting_rounds x
       WHERE x.matter = t.matter AND x.cell = 'P5' AND x.transition IS NOT NULL
         AND (x.work > t.work OR (x.work = t.work AND x.n > t.n))
         AND json_extract(x.output, '$.draft.probes') IS NOT NULL
       ORDER BY x.work DESC, x.n DESC LIMIT 1),
      json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM policy_setting_rounds t
  WHERE t.cell IN ('P4', 'P8') AND t.transition IS NOT NULL
    AND t.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers', 'session_anew')
    AND NOT EXISTS (SELECT 1 FROM policy_setting_rounds x
                    WHERE x.matter = t.matter AND x.cell IN ('P4', 'P8') AND x.transition IS NOT NULL
                      AND x.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers', 'session_anew')
                      AND (x.work > t.work OR (x.work = t.work AND x.n > t.n)));

CREATE VIEW policy_text AS
  -- The text of each setting's version of the policy being set, by the setting's matter, without its probes. Read by:
  -- cell `P5`; the engine, to check a reading of it.
  SELECT matter, work, round, policy FROM policy_draft;

CREATE VIEW policy_probes_asked AS
  -- The probes of each setting's version of the policy being set, by the setting's matter, the questions only, without
  -- the answers the architect holds right. Read by: cell `P6`, the new architect that reads the policy, which is given
  -- nothing besides them.
  SELECT d.matter,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.matter;

CREATE VIEW policy_trials AS
  -- Each setting's version of the policy being set, by the setting's matter, with every reading of it by a new
  -- architect since it was written, each by its work and round, and the architect's judgment of each, oldest first.
  -- Read by: cells `P9`, `P11` and `P12`; policy_readings; the owner's interface.
  SELECT d.matter, d.work, d.round, d.policy, d.probes,
    (SELECT json_group_array(json_object('work', r.work, 'round', r.n,
       'reading', json(json_extract(r.output, '$.outcome.reading')),
       'verdict', json((SELECT json_extract(v.output, '$.outcome.verdict') FROM policy_setting_rounds v
                        WHERE v.matter = r.matter AND v.cell = 'P7' AND v.transition IS NOT NULL
                          AND json_extract(v.output, '$.outcome.verdict') IS NOT NULL
                          AND (v.work > r.work OR (v.work = r.work AND v.n > r.n))
                        ORDER BY v.work, v.n LIMIT 1))))
     FROM policy_setting_rounds r
     WHERE r.matter = d.matter AND r.cell = 'P6' AND r.transition IS NOT NULL
       AND json_extract(r.output, '$.outcome.reading') IS NOT NULL
       AND (r.work > d.work OR (r.work = d.work AND r.n > d.round)))
       AS readings
  FROM policy_draft d;

CREATE VIEW policy_readings AS
  -- The readings of each setting's version of the policy being set by new architects since it was written, by the
  -- setting's matter, each with the architect's judgment of it, oldest first, without the text and the probes they
  -- read. Read by: cell `P7`, which weighs a reading beside every reading of the version, and cell `P8`, which fixes
  -- what they left.
  SELECT matter, readings FROM policy_trials;
