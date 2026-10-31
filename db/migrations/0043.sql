-- Takes a version 42 database to version 43: the architect's works are no longer called wakes. A work of the
-- architect's graph starts when a matter comes to the project's queue and none of its project runs, and goes on in the
-- CLI session the project's previous architect work left, which a session's start now names carried instead of wake;
-- the view of the matters each work of an architect handled is work_handled, no longer wake_handled; and the
-- descriptions of the works, the sessions, the architects' matters, the blockers, the architects' item changes and
-- the views that spoke of wakes say works, so those tables are made afresh with their rows, and those views anew. Each
-- statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW wake_handled;

DROP VIEW conversation_entries;

DROP VIEW policy_readings;

DROP VIEW policy_trials;

DROP VIEW policy_probes_asked;

DROP VIEW policy_text;

DROP VIEW policy_current;

DROP VIEW architect_queue;

CREATE TABLE works_version_42 AS SELECT * FROM works;
DROP TABLE works;

CREATE TABLE works (
  -- One run of a graph of the flow: an engineer's work on an item, one work of a project's architect, or the setting
  -- of a project's policy, a task of its own beside the architect's works. It holds the current
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
  -- For a work of the architect, or the setting of a policy, the project whose architect it is; NULL for an
  -- engineer's work, whose project is its item's. Read by: the engine, which runs one work of each project's architect
  -- at a time, and one setting of its policy, and reads the project's views for them; the architect's views; the
  -- owner's interface.
  project TEXT CHECK (project IS NULL OR graph IN ('architect', 'policy'))
) STRICT;

INSERT INTO works (id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head, paused, ended, paused_by, project)
SELECT id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head, paused, ended, paused_by, project
FROM works_version_42;
DROP TABLE works_version_42;
CREATE INDEX works_by_item ON works (item);

CREATE TABLE sessions_version_42 AS SELECT * FROM sessions;
DROP TABLE sessions;

CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session's context is past the configuration's limit, when the model its rounds run on
  -- changed, whose prompt cache it would have to write again whole, and when the owner asks for one. Each project's
  -- architect's line goes on in one CLI session across the works of the project's architect: each has a row of its
  -- own with the same cli_session, until a new policy ends it. A cell on no line has a session of its own for each
  -- round, ended once the round has.
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
SELECT id, work, line, cli, cli_session, ended, CASE WHEN started = 'wake' THEN 'carried' ELSE started END
FROM sessions_version_42;
DROP TABLE sessions_version_42;
CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;

CREATE TABLE architect_matters_version_42 AS SELECT * FROM architect_matters;
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
  -- owner_words, other_architect or slots_free. A matter handled before these kinds keeps the kind it had then.
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
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer.
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
  -- ended, saying how; or the owner answered the question in the work's group, the reply saying what. NULL otherwise.
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
FROM architect_matters_version_42;
DROP TABLE architect_matters_version_42;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;

CREATE TABLE blockers_version_42 AS SELECT * FROM blockers;
DROP TABLE blockers;

CREATE TABLE blockers (
  -- What a stopped task waits on, one row for each stop: an engineer's question, an engine tool's failure, an ask of the
  -- owner, a wait for items to deliver, or a wait for the owner's ruling on a design card. A task is an engineer's work
  -- or the setting of a policy; a work of the architect's graph is never one. A blocker belongs to the task and the cell that raised it, and has exactly one
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
FROM blockers_version_42;
DROP TABLE blockers_version_42;
CREATE UNIQUE INDEX blockers_open ON blockers (work) WHERE ended IS NULL;

CREATE TABLE architect_item_changes_version_42 AS SELECT * FROM architect_item_changes;
DROP TABLE architect_item_changes;

CREATE TABLE architect_item_changes (
  -- A change to the items a project's architect handed in while handling a matter: collected into its batch, made only
  -- once the review of the whole batch finds it sound, and kept as the log of every change made or dropped.
  -- Read by: the engine, which gives the batch to the review and makes the changes; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- The round that handed it in: an architect work's round on a matter's path, or a revision of the batch.
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

INSERT INTO architect_item_changes (id, work, round, matter, change, state, outcome)
SELECT id, work, round, matter, change, state, outcome
FROM architect_item_changes_version_42;
DROP TABLE architect_item_changes_version_42;
CREATE INDEX architect_item_changes_collected ON architect_item_changes (work) WHERE state = 'collected';

CREATE VIEW work_handled AS
  -- The matters each work of an architect handled, in the order it handled them: each matter, its kind, the cell and
  -- the round that closed it, and what that round said it did. Read by: cell `E1`; the owner's interface.
  SELECT m.handled_work AS work, m.handled_round AS round, m.id AS matter, m.kind, m.detail, m.what, r.cell,
    json_extract(r.output, '$.progress') AS said
  FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
  ORDER BY m.handled_work, m.handled_round;

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell for its question; the owner, for
  -- the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer, and whether it is urgent. Read by: cells `W0` and `Q0`; the engine, which starts the works of each
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

CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written, or put together from a correction, last before the owner's latest
  -- approval in a setting of the project's policy (in a work of its architect, before the setting was a task of its
  -- own), with the round of that approval; none before the first policy takes effect. A correction hands in only what it changes, and the engine puts the text together (`P5`); a
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
    WHERE w.graph IN ('architect', 'policy') AND a.cell IN ('P12', 'takes_effect', 'policy.takes_effect')
      AND a.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW policy_text AS
  -- The text of each work's version of the policy being set, without its probes. Read by: cell `P5`; the
  -- engine, to check a reading of it.
  SELECT work, round, policy FROM policy_draft;

CREATE VIEW policy_probes_asked AS
  -- The probes of each work's version of the policy being set, the questions only, without the answers the architect
  -- holds right. Read by: cell `P6`, the new architect that reads the policy, which is given nothing besides them.
  SELECT d.work,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.work;

CREATE VIEW policy_trials AS
  -- Each work's version of the policy being set, with every reading of it by a new architect since it was written and
  -- the architect's judgment of each, oldest first. Read by: cells `P9`, `P11` and `P12`; policy_readings; the
  -- owner's interface.
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

CREATE VIEW policy_readings AS
  -- The readings of each work's version of the policy being set by new architects since it was written, each with the
  -- architect's judgment of it, oldest first, without the text and the probes they read. Read by: cell `P7`, which
  -- weighs a reading beside every reading of the version, and cell `P8`, which fixes what they left.
  SELECT work, readings FROM policy_trials;

CREATE VIEW conversation_entries AS
  -- Every entry of the owner's conversations, gathered from the tables that hold them: a group for each engineer's
  -- work, whose members are the owner, its engineer and its project's architect (what the engineer and the architect
  -- tell and ask the owner and each other about the work, the architect's messages while it handles a matter of the
  -- work, the owner's words to either or both and replies, the design cards and the rulings on them, and the engine's
  -- notices about it); the owner's one-to-one with each project's architect (the owner's words to it, what it tells and
  -- asks the owner about no work, in its works and in the settings of its policy); one for the architects of projects
  -- the configuration connects (what they say to each other, and the owner's words to them); and one of wf-agents
  -- itself (its faults, and what the engine tells the owner about no work). The owner's words said to the architect's
  -- work in hand or to its policy setting while it runs, and its answers to them, are in the one-to-one, whatever
  -- matter it handles. A work's helpers (its advisor, review and adversarial testing) are in no group: what they and
  -- the engineer say to each other is in the work's rounds.
  -- Hidden from agents.
  -- Read by: the owner's interface, which lists the conversations, counts what waits and what is unread in each, and
  -- shows each.
  -- conversation is work:<id>, architect:<project>, architects:<project>[|<project>] (the projects' architects; the
  -- interface takes it to the group of every project the configuration connects them with), or wf-agents. source is
  -- what the entry is (message, notice, ask, interjection, words, question, report, reply, card, ruling or between) and
  -- id its row in the table it comes from; an ask is one entry, the owner's answer to it shown in it, never an entry of
  -- its own beside it. sender and recipient are a line, the owner, the engine or wf-agents, the recipient of the
  -- owner's words in a work's group the engineer, the architect or both, and in the one-to-one the line of the
  -- architect's work they were said to (architect or policy); for between, the projects. matter: the architect's
  -- matter the entry is, or holds the owner's words of. waiting: it waits for the owner (an ask that stands for an open
  -- blocker the owner holds, or an architect's ask that holds a matter, not answered; a design card not ruled while its
  -- work goes on; a fault not read). pending: the owner's words not taken up yet. undelivered: the owner's words to an
  -- agent that its work ended without taking in. answerable: an engineer's question the architect holds now, which the
  -- owner may answer in the group instead.
  SELECT CASE WHEN w.graph = 'engineer' THEN 'work:' || w.id
              WHEN r.transition = 'answers' THEN 'architect:' || w.project
              ELSE coalesce((SELECT 'work:' || x.work FROM architect_matters x JOIN works y ON y.id = x.work
                             WHERE y.graph = 'engineer' AND x.id = (
                               SELECT coalesce(json_extract(p.output, '$.outcome.pick.matter'),
                                 json_extract(p.output, '$.queue.matter'))
                               FROM rounds p
                               WHERE p.work = m.work AND p.n < m.round AND p.transition IS NOT NULL
                                 AND coalesce(json_extract(p.output, '$.outcome.pick.matter'),
                                   json_extract(p.output, '$.queue.matter')) IS NOT NULL
                               ORDER BY p.n DESC LIMIT 1)
                               AND (x.handled_round IS NULL OR x.handled_work <> m.work OR x.handled_round >= m.round)),
                            'architect:' || w.project) END AS conversation,
    coalesce(w.project, i.project) AS project, 'message' AS source, m.id AS id,
    coalesce(s.line, CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect' ELSE 'engineer' END) AS sender,
    'owner' AS recipient, m.text AS text, coalesce(r.ended_at, r.started_at) AS at, m.work AS work, m.round AS round,
    NULL AS ask, NULL AS design_card, NULL AS matter, m.read_at AS read_at, 0 AS waiting, 0 AS pending,
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
    coalesce((SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(a.body, '$.asked') q, json_each(q.value, '$.value.points') p),
             (SELECT n.text FROM engine_notices n WHERE n.ask = a.id ORDER BY n.id LIMIT 1), ''),
    coalesce((SELECT min(n.at) FROM engine_notices n WHERE n.ask = a.id), r.started_at), a.work, a.round, a.id, NULL,
    NULL, NULL,
    a.answer IS NULL AND (w.graph = 'architect' OR EXISTS (
      SELECT 1 FROM blockers b WHERE b.ask = a.id AND b.ended IS NULL AND b.holder = 'owner')), 0, 0, 0, NULL
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'interjection', j.id, j.speaker,
    CASE WHEN j.matter IS NULL THEN j.line WHEN j.informs = 1 THEN 'architect' ELSE 'both' END,
    coalesce((SELECT json_extract(x.body, '$.text') FROM architect_matters x WHERE x.id = j.matter), j.text),
    coalesce((SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)),
             (SELECT min(x.started_at) FROM rounds x WHERE x.work = j.work), 0),
    j.work, j.delivered_round, NULL, NULL, j.matter, NULL, 0,
    j.speaker = 'owner' AND ((j.informs = 0 AND j.delivered_round IS NULL AND w.ended IS NULL
        AND w.at NOT IN (CASE WHEN w.graph = 'architect' THEN 'E2' ELSE 'E1' END, '20', 'end'))
      OR EXISTS (SELECT 1 FROM architect_matters x
                 WHERE x.id = j.matter AND x.handled_round IS NULL AND x.withdrawn IS NULL)),
    j.speaker = 'owner' AND j.informs = 0 AND j.delivered_round IS NULL
      AND (w.ended IS NOT NULL OR w.at IN (CASE WHEN w.graph = 'architect' THEN 'E2' ELSE 'E1' END, '20', 'end')),
    0, NULL
  FROM interjections j JOIN works w ON w.id = j.work LEFT JOIN items i ON i.id = w.item
  WHERE j.text NOT LIKE 'Ruling on design card %'
    AND j.text NOT LIKE 'The owner confirmed every decision of design card %'
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
  WHERE c.status <> 'replaced'
  UNION ALL
  SELECT 'work:' || c.work, i.project, 'ruling', c.id, 'owner', 'engineer', '',
    coalesce(json_extract(x.value, '$.at'), cr.ended_at, cr.started_at), c.work, c.round, NULL, c.id, NULL, NULL, 0,
    0, 0, 0, NULL
  FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
  JOIN rounds cr ON cr.work = c.work AND cr.n = c.round, json_each(c.rulings) x
  WHERE c.status <> 'replaced' AND json_extract(x.value, '$.by') = 'owner'
  UNION ALL
  SELECT 'architects:' || min(json_extract(m.body, '$.from'), m.project) || '|'
      || max(json_extract(m.body, '$.from'), m.project),
    NULL, 'between', m.id, json_extract(m.body, '$.from'), m.project, coalesce(json_extract(m.body, '$.text'), m.what),
    m.at, json_extract(m.body, '$.work'), NULL, NULL, NULL, m.id, NULL, 0, 0, 0, 0, NULL
  FROM architect_matters m
  WHERE m.kind = 'other_architect' AND json_extract(m.body, '$.from') IS NOT NULL;
