-- Takes a version 28 database to version 29: a cell's name holds its stage before a dot where its stage has other
-- cells, as test.fix, and the stages take the same short words. A work and its rounds keep the ids of the flow version
-- the work ran on, so the views that read works and rounds name each cell by every id it has had, its number or code
-- and its name before as well, and the views that name a cell in what they say name it by its name now. A matter of an
-- architect held by an ask of the owner goes back, once answered, to the cell that asked, in the flow it is routed in
-- now: the cell it names, by its code or by its name before, is named afresh. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

-- The change below is the migration's, not a writer's, so the trigger that records changes to the matters is dropped
-- first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_architect_matters_update;
UPDATE architect_matters SET resume_cell = CASE resume_cell
    WHEN 'W0' THEN 'wake.start'
    WHEN 'wake' THEN 'wake.start'
    WHEN 'W1' THEN 'wake.understood'
    WHEN 'understood' THEN 'wake.understood'
    WHEN 'U1' THEN 'understand.state'
    WHEN 'state' THEN 'understand.state'
    WHEN 'U2' THEN 'understand.policy'
    WHEN 'policy_right' THEN 'understand.policy'
    WHEN 'U3' THEN 'understand.fit'
    WHEN 'fit' THEN 'understand.fit'
    WHEN 'D0' THEN 'pick.anything'
    WHEN 'anything' THEN 'pick.anything'
    WHEN 'D1' THEN 'pick.choose'
    WHEN 'pick' THEN 'pick.choose'
    WHEN 'RT' THEN 'pick.route'
    WHEN 'route' THEN 'pick.route'
    WHEN 'T1' THEN 'handle.set_straight'
    WHEN 'set_straight' THEN 'handle.set_straight'
    WHEN 'Q1' THEN 'handle.crux'
    WHEN 'crux' THEN 'handle.crux'
    WHEN 'Q2' THEN 'handle.ways'
    WHEN 'ways' THEN 'handle.ways'
    WHEN 'I1' THEN 'handle.what_is_it'
    WHEN 'what_is_it' THEN 'handle.what_is_it'
    WHEN 'I2' THEN 'handle.take_in'
    WHEN 'take_in' THEN 'handle.take_in'
    WHEN 'K1' THEN 'handle.card_sound'
    WHEN 'card_sound' THEN 'handle.card_sound'
    WHEN 'F1' THEN 'handle.have_fixed'
    WHEN 'have_fixed' THEN 'handle.have_fixed'
    WHEN 'O1' THEN 'handle.owner_words'
    WHEN 'owner_words' THEN 'handle.owner_words'
    WHEN 'M1' THEN 'handle.answer_architect'
    WHEN 'answer_architect' THEN 'handle.answer_architect'
    WHEN 'N1' THEN 'handle.what_runs'
    WHEN 'what_runs' THEN 'handle.what_runs'
    WHEN 'B1' THEN 'handle.batch_done'
    WHEN 'batch_done' THEN 'handle.batch_done'
    WHEN 'R1' THEN 'batch.review'
    WHEN 'review' THEN 'batch.review'
    WHEN 'X1' THEN 'batch.revise'
    WHEN 'revise' THEN 'batch.revise'
    WHEN 'C1' THEN 'batch.rethink'
    WHEN 'rethink' THEN 'batch.rethink'
    WHEN 'E1' THEN 'wrap_up'
    WHEN 'E2' THEN 'record'
    WHEN 'P1' THEN 'policy.study'
    WHEN 'study' THEN 'policy.study'
    WHEN 'P2' THEN 'policy.owner_reads'
    WHEN 'owner_reads' THEN 'policy.owner_reads'
    WHEN 'P3' THEN 'policy.write'
    WHEN 'write' THEN 'policy.write'
    WHEN 'P4' THEN 'policy.assemble'
    WHEN 'assemble' THEN 'policy.assemble'
    WHEN 'P5' THEN 'policy.fresh_read'
    WHEN 'fresh_read' THEN 'policy.fresh_read'
    WHEN 'P6' THEN 'policy.judge_read'
    WHEN 'judge_read' THEN 'policy.judge_read'
    WHEN 'P7' THEN 'policy.correct'
    WHEN 'correct' THEN 'policy.correct'
    WHEN 'P8' THEN 'policy.count'
    WHEN 'count' THEN 'policy.count'
    WHEN 'P11' THEN 'policy.holds_up'
    WHEN 'holds_up' THEN 'policy.holds_up'
    WHEN 'P9' THEN 'policy.owner_approves'
    WHEN 'owner_approves' THEN 'policy.owner_approves'
    WHEN 'P10' THEN 'policy.takes_effect'
    WHEN 'takes_effect' THEN 'policy.takes_effect'
    WHEN 'ASK' THEN 'ask.owner'
    WHEN 'to_owner' THEN 'ask.owner'
    WHEN 'TOOLS' THEN 'tools'
    WHEN 'S1' THEN 'search'
    ELSE resume_cell END
WHERE handled_round IS NULL AND resume_cell IS NOT NULL;

DROP VIEW observations_all;
DROP VIEW policy_trials;
DROP VIEW policy_probes_asked;
DROP VIEW policy_text;
DROP VIEW policy_draft;
DROP VIEW policy_current;
DROP VIEW architect_last_fit;
DROP VIEW architect_log;
DROP VIEW architect_state;
DROP VIEW architect_queue;
DROP VIEW advisor_rulings;
DROP VIEW adversarial_directions;
DROP VIEW item_research;
DROP VIEW work_followups;
DROP VIEW merge_queue;
DROP VIEW waiting_interjections;
DROP VIEW waited_items;
DROP VIEW work_review_record;
DROP VIEW work_measurements;
DROP VIEW work_ci;
DROP VIEW work_design_card;
DROP VIEW work_asking;
DROP VIEW work_counters;
DROP VIEW work_state;
DROP VIEW item_context;
DROP VIEW item_dependencies;
DROP VIEW parent_design_cards;
DROP VIEW item_asks;
DROP VIEW item_design_cards;
DROP VIEW item_acceptance;
DROP VIEW item_problem;
DROP VIEW item_body;
DROP VIEW item_blocked_by;

CREATE VIEW item_blocked_by AS
  -- Each item with each item it depends on by a blocking dependency that is not merged yet, as the merge cell
  -- (`merge.queue`) records a merge: no work starts on the item until every one is. A cancelled item, which never
  -- delivers, holds back none. Read by: ready_items; the architect, through its board; the owner's interface, which
  -- shows an item that would start but for them as waiting for them.
  SELECT l.item, l.depends_on, l.needs
  FROM item_links l JOIN items d ON d.id = l.depends_on
  WHERE l.kind = 'blocking' AND d.cancelled IS NULL AND NOT EXISTS (
    SELECT 1 FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = l.depends_on AND r.cell IN ('merge.queue', '9', 'merge_queue') AND r.transition = 'merged');

CREATE VIEW item_body AS
  -- The item as its creator wrote it, the acceptance it wanted included. Read by: cells `research.problem` and
  -- `test.table_review`.
  SELECT id AS item, project, title, problem, why, approach, deliver, wanted FROM items;

CREATE VIEW item_problem AS
  -- The item's problem: the engineer's understanding once cell `research.problem` has run, as written before.
  -- Read by: cell `test.blind_review`, which is given the problem and the acceptance only.
  SELECT i.id AS item, coalesce((
    SELECT json_extract(r.output, '$.outcome.understanding.problem')
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = i.id AND r.cell IN ('research.problem', '1', 'research') AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.understanding') IS NOT NULL
    ORDER BY r.work DESC, r.n DESC LIMIT 1), i.problem) AS problem
  FROM items i;

CREATE VIEW item_acceptance AS
  -- The item's locked acceptance, and the round that locked it, by its work and its number in the work; NULL until a
  -- first design passes its checks. Read by: cells `design.choose`, `implement`, `test.blind_review`,
  -- `test.table_review` and `test.adversarial`.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;

CREATE VIEW item_design_cards AS
  -- Every design card of the item's works, with its rulings. Read by: cells `research.problem`, `design.compare`,
  -- `design.choose` and `merge.gate`.
  SELECT w.item, c.work, c.id AS design_card, c.round, c.body, c.status, c.rulings, c.replaced_by
  FROM design_cards c JOIN works w ON w.id = c.work;

CREATE VIEW item_asks AS
  -- Every ask of the owner of the item's works, with its answer. Read by: cell `research.problem`.
  SELECT w.item, a.work, a.id AS ask, a.round, a.body, a.answer
  FROM owner_asks a JOIN works w ON w.id = a.work;

CREATE VIEW parent_design_cards AS
  -- The design cards of the items that depend on this one, not yet replaced: what a split-off item's parent
  -- decided. Read by: cell `design.choose`.
  SELECT l.depends_on AS item, l.item AS parent, c.id AS design_card, c.body, c.status, c.rulings
  FROM item_links l
  JOIN works w ON w.item = l.item
  JOIN design_cards c ON c.work = w.id
  WHERE c.status <> 'replaced';

CREATE VIEW item_dependencies AS
  -- What the item depends on, with the kind of each dependency, what it needs of each and how far each is: the cell
  -- its latest work is at, or why it was cancelled. Read by: cells `design.split` and `merge.gate`.
  SELECT l.item, l.depends_on, l.kind, l.needs, d.title, d.cancelled,
    (SELECT w.at FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS at,
    (SELECT w.pr FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS pr
  FROM item_links l JOIN items d ON d.id = l.depends_on;

CREATE VIEW item_context AS
  -- Where the item sits in the whole: the items above it (those that depend on it, up to the root), beside it
  -- (the other items its parents depend on) and below it (those it depends on), each with its problem and why and
  -- the kind of the dependency that brings it in: above, how it depends on the item below it on the way; beside, how
  -- their parent depends on it; below, how this item depends on it. Read by: cell `research.problem`.
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
  -- Where each work stands, as the engine keeps it. Read by: cells `test.full_ci`, `merge.queue`, `ask.back`, `tools`,
  -- `test.next_step` and `end`.
  SELECT id AS work, item, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head,
    paused, ended
  FROM works;

CREATE VIEW work_counters AS
  -- How many times each work took the rows that count each counter, by the compiled flow it runs on.
  -- Read by: cell `ask.advisor`; cell `ask.way_on`, through work_asking; the owner's interface.
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
  -- way on needs of where the work stands. Read by: cell `ask.way_on`.
  SELECT w.id AS work, w.resume_at AS asked_at,
    coalesce((SELECT c.times FROM work_counters c WHERE c.work = w.id AND c.counter = 'asks'), 0) AS asks
  FROM works w;

CREATE VIEW work_design_card AS
  -- Each work's design card as it stands: its latest one, unless that was replaced, with its rulings and the numbers of
  -- the earlier design cards of the work, which it replaced. Read by: cells `test.table_review` and `test.tree_review`.
  SELECT c.work, c.id AS design_card, c.round, c.body, c.status, c.rulings,
    (SELECT json_group_array(o.id) FROM design_cards o WHERE o.work = c.work AND o.id < c.id) AS replaces
  FROM design_cards c
  WHERE c.status <> 'replaced' AND c.id = (SELECT max(o.id) FROM design_cards o WHERE o.work = c.work);

CREATE VIEW work_ci AS
  -- The CI on each work: the latest run of the fast CI (the tool ci), of the full CI on the draft and of the tests on
  -- the code before the change (the tool ci_before), each with the head it ran on and its result, which holds the
  -- links to its runs; and the head last pushed. Read by: cells `test.table_review` and `test.tree_review`.
  SELECT w.id AS work, w.pushed_head AS head,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'ci' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS fast_ci,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'full_ci' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS full_ci,
    (SELECT json_object('head', json_extract(t.args, '$.head'), 'result', json(t.result)) FROM tool_runs t
     WHERE t.work = w.id AND t.tool = 'ci_before' AND t.result IS NOT NULL ORDER BY t.id DESC LIMIT 1) AS ci_before
  FROM works w;

CREATE VIEW work_measurements AS
  -- Every run of the measure tool for each work, with its result and the files kept. Read by: cells `test.blind_review`
  -- and `test.adversarial`.
  SELECT t.work, t.round, t.attempt, t.args, t.result, t.files
  FROM tool_runs t
  WHERE t.tool = 'measure';

CREATE VIEW work_review_record AS
  -- What decides what runs next after a review: the values rows recorded, and the last return of the table review
  -- and of adversarial testing. Read by: cell `test.next_step`.
  SELECT w.id AS work, w.recorded,
    (SELECT r.output FROM rounds r WHERE r.work = w.id AND r.cell IN ('test.table_review', '5a', 'table_review')
      AND r.transition IS NOT NULL
     ORDER BY r.n DESC LIMIT 1) AS last_review,
    (SELECT r.output FROM rounds r WHERE r.work = w.id AND r.cell IN ('test.adversarial', '6', 'adversarial')
      AND r.transition IS NOT NULL
     ORDER BY r.n DESC LIMIT 1) AS last_adversarial
  FROM works w;

CREATE VIEW waited_items AS
  -- The items each work's item depends on, with the kind of each dependency, what it needs of each, how far each is
  -- and, once merged, the last return of its merge. Read by: cells `ask.way_on` and `ask.wait_items`.
  SELECT w.id AS work, d.id AS item, l.kind, l.needs, d.title, d.cancelled,
    (SELECT x.at FROM works x WHERE x.item = d.id ORDER BY x.id DESC LIMIT 1) AS at,
    (SELECT r.output FROM rounds r JOIN works x ON x.id = r.work
     WHERE x.item = d.id AND r.cell IN ('merge.queue', '9', 'merge_queue') AND r.transition = 'merged'
     ORDER BY r.work DESC, r.n DESC LIMIT 1) AS merged
  FROM works w JOIN item_links l ON l.item = w.item JOIN items d ON d.id = l.depends_on;

CREATE VIEW waiting_interjections AS
  -- Interjections not yet handed in, in the order they came. Read by: every agent cell; cells `test.next_step`,
  -- `merge.gate` and `end`.
  SELECT work, id AS interjection, line, speaker, text FROM interjections WHERE delivered_round IS NULL;

CREATE VIEW merge_queue AS
  -- Each project's merge queue: the works at the merge cell, oldest first. Read by: cell `merge.queue`.
  SELECT i.project, w.id AS work, w.pr, w.pushed_head
  FROM works w JOIN items i ON i.id = w.item
  WHERE w.at IN ('merge.queue', '9', 'merge_queue') AND w.ended IS NULL
  ORDER BY w.id;

CREATE VIEW work_followups AS
  -- What to do next, as each report or fix handed in. A return of a flow version before the answer of implementing
  -- and fixing was flat holds it under end. Read by: cell `end`, whose step queues a report's latest for the architect;
  -- the owner's interface.
  SELECT w.item, r.work, r.n AS round, coalesce(json_extract(r.output, '$.outcome.followup'),
    json_extract(r.output, '$.outcome.end.followup')) AS followup
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE r.cell IN ('implement', '4', 'test.fix', '8', 'fix') AND coalesce(json_extract(r.output, '$.outcome.followup'),
    json_extract(r.output, '$.outcome.end.followup')) IS NOT NULL;

CREATE VIEW item_research AS
  -- The research and design each work wrote for its item: every accepted own answer of the research, design and
  -- implementation cells. Read by: the owner's interface; the architect.
  SELECT w.item, r.work, r.n AS round, r.cell, json_extract(r.output, '$.outcome') AS outcome
  FROM rounds r JOIN works w ON w.id = r.work
  WHERE r.cell IN ('research.problem', '1', 'research', 'design.compare', '2', 'compare', 'design.choose', '2a',
    'choose', 'design.cost', '3c', 'cost', 'implement', '4', 'test.fix', '8', 'fix')
    AND r.transition IS NOT NULL;

CREATE VIEW adversarial_directions AS
  -- The directions each adversarial round tried. Read by: the owner's interface, to see what kinds of work it
  -- finds problems in.
  SELECT w.item, r.work, r.n AS round, d.value AS direction
  FROM rounds r JOIN works w ON w.id = r.work, json_each(r.output, '$.outcome.directions') d
  WHERE r.cell IN ('test.adversarial', '6', 'adversarial');

CREATE VIEW advisor_rulings AS
  -- Disputes the fix took to the advisor, and the ruling it carried out. Read by: the owner's interface.
  SELECT r.work, r.n AS round, json_extract(x.value, '$.id') AS finding, json_extract(x.value, '$.ruling') AS ruling
  FROM rounds r, json_each(r.output, '$.outcome.rows') x
  WHERE r.cell IN ('test.fix', '8', 'fix') AND json_extract(x.value, '$.answer') = 'advisor_ruled';

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell for its question; the owner, for
  -- the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer, and whether it is urgent. Read by: cells `wake.start` and `pick.anything`; the engine, which wakes each
  -- project's architect and gives the architect its queue; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('ask.architect', '15', 'to_architect')
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

CREATE VIEW architect_state AS
  -- Each project's state of things as its architect last wrote it: its latest understanding (cell `understand.state`),
  -- with what its wrap-ups learned and every note written since; a round that asked for tools or paused wrote none of
  -- them. Read by: cells `understand.state` and `wrap_up`; the owner's interface.
  SELECT project, work, round, state, direction, learned, notes FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.state') AS state,
      json_extract(r.output, '$.outcome.direction') AS direction,
      (SELECT json_group_array(json_extract(x.output, '$.outcome.learned')) FROM rounds x JOIN works y ON y.id = x.work
       WHERE y.graph = 'architect' AND y.project = w.project AND x.cell IN ('wrap_up', 'E1')
         AND x.transition IS NOT NULL
         AND x.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
         AND (x.work > r.work OR (x.work = r.work AND x.n > r.n))) AS learned,
      (SELECT json_group_array(json_object('item', n.item, 'text', n.text)) FROM architect_notes n
       WHERE n.project = w.project AND (n.work > r.work OR (n.work = r.work AND n.round > r.n))) AS notes,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('understand.state', 'U1', 'state') AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner'))
  WHERE k = 1;

CREATE VIEW architect_log AS
  -- Each project's architect's log, newest first, its latest forty entries: each judgment it handed in (whether the
  -- policy is right, whether what runs fits, each matter's answer, each rethink after a batch) and the owner's answer
  -- to each study of a new policy, by the round that gave it, with the matter it handled and what it said, its
  -- conclusion with its reasons. Read by: cells `understand.state`, `understand.policy` and `understand.fit`.
  SELECT project, work, round, cell, at, matter, said FROM (
    SELECT w.project, r.work, r.n AS round, r.cell, r.ended_at AS at,
      (SELECT m.id FROM architect_matters m WHERE m.handled_work = r.work AND m.handled_round = r.n) AS matter,
      coalesce(json_extract(r.output, '$.progress'),
        json_extract(r.output, '$.ruling.choice') || ': ' || json_extract(r.output, '$.ruling.text')) AS said,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
      AND r.cell IN ('understand.policy', 'U2', 'policy_right', 'understand.fit', 'U3', 'fit', 'handle.set_straight',
        'T1', 'set_straight', 'handle.ways', 'Q2', 'ways', 'handle.take_in', 'I2', 'take_in', 'handle.card_sound', 'K1',
        'card_sound', 'handle.have_fixed', 'F1', 'have_fixed', 'handle.owner_words', 'O1', 'owner_words',
        'handle.answer_architect', 'M1', 'answer_architect', 'handle.what_runs', 'N1', 'what_runs', 'batch.rethink',
        'C1', 'rethink', 'policy.owner_reads', 'P2', 'owner_reads'))
  WHERE k <= 40;

CREATE VIEW architect_last_fit AS
  -- Each project's architect's latest answer to whether what runs fits. Read by: cell `understand.fit`.
  SELECT project, work, round, fit FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.fit') AS fit,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('understand.fit', 'U3', 'fit') AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.fit') IS NOT NULL)
  WHERE k = 1;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written, or put together from a correction, last before the owner's latest
  -- approval in a wake of the project's architect, with the round of that approval; none before the first policy takes
  -- effect. A correction hands in only what it changes, and the engine puts the text together (`policy.assemble`); a
  -- correction of a flow version before that held the whole text. Older versions stay readable in the rounds and are
  -- given to no agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
         json_extract(t.output, '$.outcome.revision.policy'))
       FROM rounds t
       WHERE t.work = a.work AND t.cell IN ('policy.write', 'P3', 'write', 'policy.assemble', 'P4', 'assemble',
         'policy.correct', 'P7', 'correct')
         AND t.transition IS NOT NULL AND t.n < a.n
         AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
       ORDER BY t.n DESC LIMIT 1) AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY a.work DESC, a.n DESC) AS k
    FROM rounds a JOIN works w ON w.id = a.work
    WHERE w.graph = 'architect' AND a.cell IN ('policy.takes_effect', 'P10', 'takes_effect')
      AND a.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW policy_draft AS
  -- Each wake's version of the policy being set: the round of the latest writing or correcting in the wake, the text as
  -- it stands, and the probes with the answers the architect holds right. A correction hands in only what it changes,
  -- and the text is then as the engine last put it together (`policy.assemble`); a correction of a flow version before
  -- that held the whole text. Read by: policy_text, policy_probes_asked and policy_trials; the cells on the architect's
  -- line take it from the returns that wrote it or put it together.
  SELECT t.work, t.n AS round,
    (SELECT coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
       json_extract(x.output, '$.outcome.revision.policy'))
     FROM rounds x
     WHERE x.work = t.work AND x.cell IN ('policy.write', 'P3', 'write', 'policy.assemble', 'P4', 'assemble',
       'policy.correct', 'P7', 'correct') AND x.transition IS NOT NULL
       AND coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
         json_extract(x.output, '$.outcome.revision.policy')) IS NOT NULL
     ORDER BY x.n DESC LIMIT 1) AS policy,
    coalesce(json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM rounds t JOIN works w ON w.id = t.work
  WHERE w.graph = 'architect' AND t.cell IN ('policy.write', 'P3', 'write', 'policy.correct', 'P7', 'correct')
    AND t.transition IS NOT NULL
    AND t.n = (SELECT max(x.n) FROM rounds x
               WHERE x.work = t.work AND x.cell IN ('policy.write', 'P3', 'write', 'policy.correct', 'P7', 'correct')
                 AND x.transition IS NOT NULL);

CREATE VIEW policy_text AS
  -- The text of each wake's version of the policy being set, without its probes. Read by: cell `policy.assemble`; the
  -- engine, to check a reading of it.
  SELECT work, round, policy FROM policy_draft;

CREATE VIEW policy_probes_asked AS
  -- The probes of each wake's version of the policy being set, the questions only, without the answers the architect
  -- holds right. Read by: cell `policy.fresh_read`, the new architect that reads the policy.
  SELECT d.work, d.round,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.work, d.round;

CREATE VIEW policy_trials AS
  -- Each wake's version of the policy being set, with every reading of it by a new architect since it was written and
  -- the architect's judgment of each, oldest first. Read by: cells `policy.count`, `policy.owner_approves` and
  -- `policy.takes_effect`; the owner's interface.
  SELECT d.work, d.round, d.policy, d.probes,
    (SELECT json_group_array(json_object('round', r.n, 'reading', json(json_extract(r.output, '$.outcome.reading')),
       'verdict', json((SELECT json_extract(v.output, '$.outcome.verdict') FROM rounds v
                        WHERE v.work = r.work AND v.cell IN ('policy.judge_read', 'P6', 'judge_read')
                          AND v.transition IS NOT NULL
                          AND v.n > r.n
                        ORDER BY v.n LIMIT 1))))
     FROM rounds r
     WHERE r.work = d.work AND r.cell IN ('policy.fresh_read', 'P5', 'fresh_read')
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
    WHERE r.cell IN ('merge.queue', '9', 'merge_queue')
      AND r.transition = 'merged' GROUP BY w.item) m ON m.item = o.folded_into;
