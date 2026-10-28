-- Takes a version 13 database to version 14: the architect's flow runs. The architect's matters are a table of their
-- own, what holds new works back and the plan's segments, gates and budgets are tables, an item may hang on a segment,
-- say what it touches, keep the acceptance its creator wanted and be held; a session can be ended; the dispatch order
-- comes from the owner's queue and then from the plan; and the views the architect's cells read are made, with the
-- views whose definitions or descriptions changed made afresh. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

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
  -- For a next look the architect set for a time, that time; NULL for a matter due at once. A matter not due wakes no
  -- one and is not in the queue.
  due INTEGER,
  -- For a next look the architect set on an item's work ending: that item; the matter is due once the item's latest
  -- work has ended or reached its end. NULL for any other.
  due_item INTEGER REFERENCES items(id),
  -- The held matter an owner's answer is for: a matter of the kind owner_words that answers an ask the architect held
  -- a matter with. NULL for any other. Read by: the architect, through the matter's material.
  answers INTEGER REFERENCES architect_matters(id),
  -- Why the matter waits though picked: owner_ask while an ask of the owner the eighth question put holds it, approval
  -- while changes the architect collected for it wait for the owner's approval; NULL while it does not. A held matter
  -- is handled at the round that held it, and the owner's answer comes back as a matter of its own.
  held TEXT CHECK (held IN ('owner_ask', 'approval')),
  -- The ask of the owner that holds it; for changes held for approval, NULL until the wrap-up puts them to the owner.
  -- Read by: the engine, which queues the owner's answer as a matter of its own.
  held_by INTEGER REFERENCES owner_asks(id),
  -- The architect's reply to an engineer's question, as engine_op reply handed it in, JSON; NULL until it replied.
  -- Read by: the engine, which takes it back unchanged to the work that asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- The architect's round that handled it: its wake and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK (held IS NOT NULL OR held_by IS NULL),
  CHECK (held IS NOT 'owner_ask' OR held_by IS NOT NULL),
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
  since INTEGER NOT NULL,
  -- Why, in the owner's language, as the architect said it for its own hold; NULL for the others, whose reason says
  -- why. Read by: the owner's interface.
  why TEXT
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

ALTER TABLE items ADD COLUMN segment TEXT REFERENCES plan_segments(key);
ALTER TABLE items ADD COLUMN touches TEXT CHECK (touches IS NULL OR json_valid(touches));
ALTER TABLE items ADD COLUMN wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted));
ALTER TABLE items ADD COLUMN held TEXT;

-- A session can be ended, so that the architect's line goes on in one CLI session across its wakes until a new policy
-- ends it, and a cell on no line has a session of its own for each round: the sessions are made afresh with their rows,
-- each line of a work keeping one session going on.
CREATE TABLE sessions_version_13 AS SELECT * FROM sessions;
DROP TABLE sessions;
CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line keeps one session for the whole work and lets the CLI compact
  -- it; after a compaction the engine gives the cell's rules and inputs again. The architect's line goes on in one CLI
  -- session across its wakes: each wake has a row of its own with the same cli_session, until a new policy ends it. A
  -- cell on no line has a session of its own for each round, ended once the round has.
  -- Hidden from agents.
  -- Read by: the engine, to resume a line.
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
  ended INTEGER
) STRICT;
INSERT INTO sessions (id, work, line, cli, cli_session) SELECT id, work, line, cli, cli_session FROM sessions_version_13;
DROP TABLE sessions_version_13;
-- The session each line of a work goes on in: one at most, among those not ended.
CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;

-- The architect's matters not handled.
CREATE INDEX architect_matters_waiting ON architect_matters (id) WHERE handled_round IS NULL;

-- Every design card not replaced that the owner has not confirmed is for the architect to look at, as a card handed
-- in from now on is: whether the card holds and the approaches it chose are reasonable; the owner alone rules on it.
-- A card is confirmed when the owner's last ruling confirms every decision: a verdict of confirm on each of them, or,
-- for a ruling made before rulings were per decision, on the card as a whole.
INSERT INTO architect_matters (kind, what, body, item, work, round, at)
  SELECT 'design_card', 'Design card ' || c.id || ' of work ' || c.work || ' to look at',
    json_object('design_card', c.id), w.item, c.work, c.round, CAST(unixepoch('subsec') * 1000 AS INTEGER)
  FROM design_cards c JOIN works w ON w.id = c.work
  WHERE c.status <> 'replaced' AND NOT coalesce((
    SELECT CASE
      WHEN json_type(r.value, '$.decisions') = 'array' THEN
        NOT EXISTS (
          SELECT 1 FROM json_each(r.value, '$.decisions') d WHERE json_extract(d.value, '$.verdict') IS NOT 'confirm')
        AND (SELECT count(DISTINCT json_extract(d.value, '$.decision')) FROM json_each(r.value, '$.decisions') d
             WHERE json_extract(d.value, '$.decision') BETWEEN 1 AND max(1, coalesce(json_array_length(c.body), 1)))
          = max(1, coalesce(json_array_length(c.body), 1))
      ELSE json_extract(r.value, '$.verdict') = 'confirm' END
    FROM json_each(c.rulings) r WHERE json_extract(r.value, '$.by') = 'owner' ORDER BY r.key DESC LIMIT 1), 0);

-- The side reports rounds handed in before the architect ran are one matter for it, to look through them all, each
-- with the round that handed it in and its place in that round's list; none when there are none.
INSERT INTO architect_matters (kind, what, body, at)
  SELECT 'other_question', 'Look through the ' || n || ' side reports handed in before you ran',
    json_object('side_reports', json(reports)), CAST(unixepoch('subsec') * 1000 AS INTEGER)
  FROM (
    SELECT count(*) AS n, json_group_array(json_object('work', work, 'round', round, 'cell', cell, 'index', idx,
      'report', CASE WHEN type IN ('object', 'array') THEN json(report) ELSE report END)) AS reports
    FROM (
      SELECT r.work, r.n AS round, r.cell, s.key AS idx, s.type, s.value AS report
      FROM rounds r, json_each(r.output, '$.side_reports') s
      WHERE r.output IS NOT NULL
      ORDER BY r.work, r.n, s.key))
  WHERE n > 0;

DROP VIEW ready_items;
DROP VIEW item_body;
DROP VIEW side_reports;
DROP VIEW item_design_cards;

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

CREATE VIEW side_reports AS
  -- Every side report a round handed in. Read by: the engine, which queues each for the architect; the owner's
  -- interface.
  SELECT r.work, r.n AS round, r.cell, s.value AS report
  FROM rounds r, json_each(r.output, '$.side_reports') s
  WHERE r.output IS NOT NULL;

CREATE VIEW item_design_cards AS
  -- Every design card of the item's works, with its rulings. Read by: cells 1, 2, 2a and 19.
  SELECT w.item, c.work, c.id AS design_card, c.round, c.body, c.status, c.rulings, c.replaced_by
  FROM design_cards c JOIN works w ON w.id = c.work;

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

CREATE VIEW urgent_side_reports AS
  -- The side reports marked urgent that came while each wake of the architect runs and are not handled yet: an urgent
  -- report is given to the running wake at once, at the top of its next round, as well as waiting in the queue.
  -- Read by: every cell of the architect's line.
  SELECT w.id AS work, m.id AS matter, m.item, m.what, json_extract(m.body, '$.report') AS report
  FROM works w JOIN architect_matters m
    ON m.at >= (SELECT min(r.started_at) FROM rounds r WHERE r.work = w.id)
  WHERE w.graph = 'architect' AND w.ended IS NULL AND m.handled_round IS NULL
    AND json_extract(m.body, '$.urgent') = 1;
