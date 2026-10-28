-- Takes a version 19 database to version 20: one architect for each project, on the architect's new graph. A wake of the
-- architect is for a project; each project has its queue of matters, which take the kinds of the new graph, its holds
-- on new starts and its policy; item changes are collected in batches and made once reviewed; and observations are
-- recorded beside the items. Each statement below that defines something is written exactly as db/schema.sql writes
-- it.

-- The changes below are the migration's, not a writer's: the triggers that record changes to the tables it updates are
-- dropped first, and every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_works_update;
DROP TRIGGER IF EXISTS track_owner_asks_update;
DROP TRIGGER IF EXISTS track_sessions_update;

ALTER TABLE works ADD COLUMN project TEXT CHECK (project IS NULL OR graph = 'architect');

-- A wake before was over every project: in a database of one project, it was that project's.
UPDATE works SET project = (SELECT min(project) FROM items)
WHERE graph = 'architect' AND (SELECT count(DISTINCT project) FROM items) = 1;

-- A wake still open is at a cell of the old graph: it ends, with the round it was in, and the next wake starts from
-- the new graph's start in a new session.
CREATE TABLE wakes_ended AS SELECT id FROM works WHERE graph = 'architect' AND ended IS NULL AND at <> 'A12';
UPDATE rounds SET ended_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
  rejected = json_object('interrupted', 'the architect''s graph changed, and the next wake starts from its start')
WHERE ended_at IS NULL AND work IN (SELECT id FROM wakes_ended);
UPDATE works SET ended = 'stopped: the architect''s graph changed, and the next wake starts from its start'
WHERE id IN (SELECT id FROM wakes_ended);
UPDATE sessions SET ended = CAST(unixepoch('subsec') * 1000 AS INTEGER) WHERE line = 'architect' AND ended IS NULL;

-- The asks those wakes waited for need no answer any longer, and nor does an ask of the changes the architect
-- collected for the owner's approval: its matters go back to the queue, and their changes come back as item changes a
-- review looks at. An ask that holds a matter, or that held no wake, stays for the owner to answer.
UPDATE owner_asks SET answer = json_object('by', 'engine', 'at', CAST(unixepoch('subsec') * 1000 AS INTEGER), 'text',
    'No answer is needed: the architect''s graph changed, and its next wake starts from its start.')
WHERE answer IS NULL AND json_extract(body, '$.from') = 'architect' AND (
  json_extract(body, '$.kind') = 'changes'
  OR (work IN (SELECT id FROM wakes_ended) AND json_extract(body, '$.holds') IS NULL
      AND coalesce(json_extract(body, '$.kind'), '') <> 'open'));
DROP TABLE wakes_ended;

-- The matters, each for a project: its item's, or the project of the work that raised it; in a database of one project,
-- that project. A matter not handled takes the kind of the new graph its kind was, with its old kind as its detail;
-- one held for changes waiting for approval goes back to the queue, and one held by an ask still waiting stays held,
-- to go back to the cell of the new graph that decides it. A matter handled stays as it was.
DROP VIEW architect_queue;
DROP VIEW architect_state;
DROP VIEW architect_log;
DROP VIEW architect_last_fit;
DROP VIEW wake_handled;
DROP VIEW wake_changes;
DROP VIEW policy_current;
DROP VIEW urgent_side_reports;
CREATE TABLE architect_matters_version_19 AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;

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
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO architect_matters (id, project, kind, detail, what, body, item, work, round, at, asked, resume_cell, reply,
    handled_work, handled_round)
  WITH old AS (
    SELECT m.*,
      coalesce(
        (SELECT i.project FROM items i WHERE i.id = m.item),
        (SELECT i.project FROM works w JOIN items i ON i.id = w.item WHERE w.id = m.work),
        (SELECT min(project) FROM items)) AS project,
      m.held = 'approval' OR (m.held = 'owner_ask'
        AND EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.held_by AND a.answer IS NULL)) AS released,
      m.held = 'owner_ask'
        AND EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.held_by AND a.answer IS NULL) AS still_held
    FROM architect_matters_version_19 m),
  mapped AS (
    SELECT old.*, (handled_round IS NULL OR released) AS open,
      CASE
        WHEN kind IN ('split', 'duplicate', 'depends_on_other_item', 'out_of_scope_defect', 'root_elsewhere',
                      'too_costly', 'other_question') AND (json_extract(body, '$.report') IS NOT NULL
                      OR json_extract(body, '$.side_reports') IS NOT NULL) THEN 'report'
        WHEN kind IN ('split', 'duplicate', 'depends_on_other_item', 'out_of_scope_defect', 'root_elsewhere',
                      'too_costly', 'other_question') THEN 'engineer_question'
        WHEN kind IN ('tool_failed', 'environment_changed') THEN 'failure'
        WHEN kind IN ('owner_words', 'owner_approved') THEN 'owner_words'
        WHEN kind IN ('constitution_violation', 'judgment_mismatch', 'running_unfit', 'tidy_board', 'next_look',
                      'policy_changed') THEN 'set_straight'
        WHEN kind = 'advance' THEN 'slots_free'
        WHEN kind = 'report_followup' THEN 'report'
        ELSE kind END AS new_kind,
      CASE
        WHEN json_extract(body, '$.report') IS NOT NULL THEN json_extract(body, '$.report.kind')
        WHEN json_extract(body, '$.side_reports') IS NOT NULL THEN 'other'
        WHEN kind = 'report_followup' THEN 'followup'
        WHEN kind = 'owner_approved' THEN 'approved_changes'
        WHEN kind = 'owner_words' AND json_extract(body, '$.owner_ask') IS NOT NULL THEN 'answer'
        WHEN kind = 'owner_words' THEN 'said'
        WHEN kind IN ('design_card', 'advance') THEN NULL
        ELSE kind END AS new_detail
    FROM old)
  SELECT id, project,
    CASE WHEN open THEN new_kind ELSE kind END,
    CASE WHEN open THEN new_detail END,
    what, body, item, work, round, at,
    CASE WHEN still_held THEN held_by END,
    CASE WHEN still_held THEN
      CASE new_kind WHEN 'engineer_question' THEN 'Q2' WHEN 'report' THEN 'I2' WHEN 'set_straight' THEN 'T1'
        WHEN 'design_card' THEN 'K1' WHEN 'failure' THEN 'F1' WHEN 'owner_words' THEN 'O1' WHEN 'slots_free' THEN 'N1'
        ELSE 'O1' END END,
    reply,
    CASE WHEN released THEN NULL ELSE handled_work END,
    CASE WHEN released THEN NULL ELSE handled_round END
  FROM mapped
  WHERE project IS NOT NULL
  ORDER BY id;

DROP TABLE architect_matters_version_19;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;

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

-- New starts are held for each project: the architect's own hold holds every project it held before; a hold of a
-- wake that ended (setting the whole straight, setting a policy) ends with it.
CREATE TABLE dispatch_holds_version_19 AS SELECT * FROM dispatch_holds;
DROP TABLE dispatch_holds;

CREATE TABLE dispatch_holds (
  -- What stops new works of a project from starting, one row for each: its architect's own hold, its architect setting
  -- it straight, its architect setting a new policy. While a project has a row here, the engine starts no new work on
  -- its items; works already running go on, and the other projects' works start as usual.
  -- Read by: the engine at cell 0; the architect, through its board; the owner's interface.
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


INSERT INTO dispatch_holds (project, reason, since, why)
  SELECT p.project, h.reason, h.since, h.why FROM dispatch_holds_version_19 h, (SELECT DISTINCT project FROM items) p
  WHERE h.reason = 'architect';
DROP TABLE dispatch_holds_version_19;

CREATE INDEX architect_item_changes_collected ON architect_item_changes (work) WHERE state = 'collected';
CREATE INDEX observations_by_project ON observations (project);

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled and not held by an ask of the owner still waiting, each with who
  -- waits for it (an agent: an engineer stopped at the architect's cell for its question; the owner, for the owner's
  -- words; or no one), when it came and how long it has waited, whether it comes back with the owner's answer, and
  -- whether it is urgent. Read by: cells W0 and D0; the engine, which wakes each project's architect and gives the
  -- architect its queue; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w WHERE w.id = m.work AND w.graph = 'engineer' AND w.at = '15' AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AS answered,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL
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
  -- Each project's state of things as its architect last wrote it: its latest understanding (cell U1), with what its
  -- wrap-ups learned and every note written since. Read by: cells U1 and E1; the owner's interface.
  SELECT project, work, round, state, direction, learned, notes FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.state') AS state,
      json_extract(r.output, '$.outcome.direction') AS direction,
      (SELECT json_group_array(json_extract(x.output, '$.outcome.learned')) FROM rounds x JOIN works y ON y.id = x.work
       WHERE y.graph = 'architect' AND y.project = w.project AND x.cell = 'E1' AND x.transition IS NOT NULL
         AND (x.work > r.work OR (x.work = r.work AND x.n > r.n))) AS learned,
      (SELECT json_group_array(json_object('item', n.item, 'text', n.text)) FROM architect_notes n
       WHERE n.project = w.project AND (n.work > r.work OR (n.work = r.work AND n.round > r.n))) AS notes,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell = 'U1' AND r.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW architect_log AS
  -- Each project's architect's log, newest first, its latest forty entries: each judgment it handed in (whether the
  -- policy is right, whether what runs fits, each matter's answer, each rethink after a batch) and the owner's answer
  -- to each study of a new policy, by the round that gave it, with the matter it handled and what it said, its
  -- conclusion with its reasons. Read by: cells U1, U2 and U3.
  SELECT project, work, round, cell, at, matter, said FROM (
    SELECT w.project, r.work, r.n AS round, r.cell, r.ended_at AS at,
      (SELECT m.id FROM architect_matters m WHERE m.handled_work = r.work AND m.handled_round = r.n) AS matter,
      coalesce(json_extract(r.output, '$.progress'),
        json_extract(r.output, '$.ruling.choice') || ': ' || json_extract(r.output, '$.ruling.text')) AS said,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
      AND r.cell IN ('U2', 'U3', 'T1', 'Q2', 'I2', 'K1', 'F1', 'O1', 'M1', 'N1', 'C1', 'P2'))
  WHERE k <= 40;

CREATE VIEW architect_last_fit AS
  -- Each project's architect's latest answer to whether what runs fits. Read by: cell U3.
  SELECT project, work, round, fit FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.fit') AS fit,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell = 'U3' AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.fit') IS NOT NULL)
  WHERE k = 1;

CREATE VIEW wake_handled AS
  -- The matters each wake of an architect handled, in the order it handled them: each matter, its kind, the cell and
  -- the round that closed it, and what that round said it did. Read by: cell E1; the owner's interface.
  SELECT m.handled_work AS work, m.handled_round AS round, m.id AS matter, m.kind, m.detail, m.what, r.cell,
    json_extract(r.output, '$.progress') AS said
  FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
  ORDER BY m.handled_work, m.handled_round;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written or corrected last before the owner's latest approval in a wake
  -- of the project's architect, with the round of that approval; none before the first policy takes effect. Older
  -- versions stay readable in the rounds and are given to no agent. Read by: the engine, for the architect's cells;
  -- the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      (SELECT coalesce(json_extract(t.output, '$.outcome.policy'), json_extract(t.output, '$.outcome.revision.policy'))
       FROM rounds t WHERE t.work = a.work AND t.cell IN ('P3', 'P7') AND t.transition IS NOT NULL AND t.n < a.n
       ORDER BY t.n DESC LIMIT 1) AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY a.work DESC, a.n DESC) AS k
    FROM rounds a JOIN works w ON w.id = a.work
    WHERE w.graph = 'architect' AND a.cell = 'P10' AND a.transition IS NOT NULL)
  WHERE k = 1;

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
    WHERE r.cell = '9' AND r.transition = 'merged' GROUP BY w.item) m ON m.item = o.folded_into;

CREATE VIEW observations_open AS
  -- Each project's open observations, each with its number, what it is, its tags, how many times it was reported,
  -- where it came from, what it refers to and the item that took it in, if one has. Read by: cell S1, which searches
  -- them; the engine.
  SELECT project, id, what, tags, times_reported, source, refs, folded_into
  FROM observations_all WHERE closed IS NULL;
