-- Takes a version 36 database to version 37: the setting of a project's policy is a task of its own, a work of the
-- graph policy beside the project's wakes, which may name the project as a wake does; and the views of the policy in
-- force, the policy being set, the architect's log and the owner's conversations read the setting from its own works,
-- as from the wakes of the versions before. The works are made afresh with their rows for the graph's new name. Each
-- statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE works_version_36 AS SELECT * FROM works;
DROP TABLE works;

CREATE TABLE works (
  -- One run of a graph of the flow: an engineer's work on an item, one wake of a project's architect, or the setting
  -- of a project's policy, a task of its own beside the wakes. It holds the current
  -- state the engine needs to take the next step, written in the same transaction as the round that changed it; the
  -- history is in rounds. Which cell brought the work to its cell, whether it is done, what it waits for and when it
  -- started and ended are derived from its rounds. At most one work per item runs at a time.
  -- Read by: the engine at every step.
  id INTEGER PRIMARY KEY,
  -- The item worked on; NULL for an architect wake, which belongs to no item but to its project.
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
  -- For a wake of the architect, or the setting of a policy, the project whose architect it is; NULL for an
  -- engineer's work, whose project is its item's. Read by: the engine, which runs one wake of each project's architect
  -- at a time, and one setting of its policy, and reads the project's views for them; the architect's views; the
  -- owner's interface.
  project TEXT CHECK (project IS NULL OR graph IN ('architect', 'policy'))
) STRICT;

INSERT INTO works (id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head, paused, ended, paused_by, project)
SELECT id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, pushed_head, paused, ended, paused_by, project
FROM works_version_36;
DROP TABLE works_version_36;
CREATE INDEX works_by_item ON works (item);

DROP VIEW conversation_entries;
DROP VIEW architect_log;
DROP VIEW policy_readings;
DROP VIEW policy_trials;
DROP VIEW policy_probes_asked;
DROP VIEW policy_text;
DROP VIEW policy_draft;
DROP VIEW policy_current;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written, or put together from a correction, last before the owner's latest
  -- approval in a setting of the project's policy (in a wake of its architect, before the setting was a task of its
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

CREATE VIEW policy_draft AS
  -- Each setting's version of the policy being set: the round of the latest writing or correcting in it, the text as
  -- it stands, and the probes with the answers the architect holds right. A correction hands in only what it changes,
  -- its edits and the probes it adds or changes beside those it keeps, and the text and the probes are then as the
  -- engine last put them together (`P5`), as the text is after a minor fix a reading found, which leaves the version
  -- as it was; a correction of a flow version before that held the whole text, or the whole probes. Read by:
  -- policy_text, policy_probes_asked and policy_trials; the cells on the architect's line take it from the returns that
  -- wrote it or put it together.
  SELECT t.work, t.n AS round,
    (SELECT coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
       json_extract(x.output, '$.outcome.revision.policy'))
     FROM rounds x
     WHERE x.work = t.work AND x.cell IN ('P4', 'write', 'policy.write', 'P5', 'assemble', 'policy.assemble', 'P8',
       'correct', 'policy.correct') AND x.transition IS NOT NULL
       AND coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
         json_extract(x.output, '$.outcome.revision.policy')) IS NOT NULL
     ORDER BY x.n DESC LIMIT 1) AS policy,
    coalesce(
      (SELECT json_extract(x.output, '$.draft.probes') FROM rounds x
       WHERE x.work = t.work AND x.cell = 'P5' AND x.transition IS NOT NULL AND x.n > t.n
         AND json_extract(x.output, '$.draft.probes') IS NOT NULL
       ORDER BY x.n DESC LIMIT 1),
      json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM rounds t JOIN works w ON w.id = t.work
  WHERE w.graph IN ('architect', 'policy') AND t.cell IN ('P4', 'write', 'policy.write', 'P8', 'correct', 'policy.correct')
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
  -- holds right. Read by: cell `P6`, the new architect that reads the policy, which is given nothing besides them.
  SELECT d.work,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.work;

CREATE VIEW policy_trials AS
  -- Each wake's version of the policy being set, with every reading of it by a new architect since it was written and
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
  -- The readings of each wake's version of the policy being set by new architects since it was written, each with the
  -- architect's judgment of it, oldest first, without the text and the probes they read. Read by: cell `P7`, which
  -- weighs a reading beside every reading of the version, and cell `P8`, which fixes what they left.
  SELECT work, readings FROM policy_trials;

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
    WHERE w.graph IN ('architect', 'policy') AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
      AND r.cell IN ('P1', 'U2', 'policy_right', 'understand.policy', 'N1', 'U3', 'fit', 'understand.fit', 'H1', 'T1',
        'set_straight', 'handle.set_straight', 'H3', 'ways', 'handle.ways', 'H5', 'I2', 'take_in', 'handle.take_in',
        'H6', 'K1', 'card_sound', 'handle.card_sound', 'H7', 'F1', 'have_fixed', 'handle.have_fixed', 'H8', 'O1',
        'owner_words', 'handle.owner_words', 'H9', 'M1', 'answer_architect', 'handle.answer_architect', 'H10',
        'what_runs', 'handle.what_runs', 'B4', 'C1', 'rethink', 'batch.rethink', 'P3', 'owner_reads',
        'policy.owner_reads'))
  WHERE k <= 40;

CREATE VIEW conversation_entries AS
  -- Every entry of the owner's conversations, gathered from the tables that hold them: one conversation for each
  -- engineer's work (what its agents tell and ask the owner, the owner's answers and interjections, its asks of the
  -- architect and the advisor with their replies, and the engine's notices about it), one for each project's architect
  -- (the owner's words, what it tells and asks the owner, in its wakes and in the settings of its policy, and the
  -- design cards of the project's works for the owner to rule on), one for each pair of projects' architects (their messages), and one of wf-agents itself (its faults, and
  -- what the engine tells the owner about no work).
  -- Hidden from agents.
  -- Read by: the owner's interface, which lists the conversations and shows each.
  -- conversation is work:<id>, architect:<project>, architects:<project>|<project> in order, or wf-agents. source is
  -- what the entry is (message, notice, ask, answer, interjection, words, question, reply, advice_ask, advice, card,
  -- ruling or between) and id its row in the table it comes from, for advice_ask and advice the round. sender and
  -- recipient are a line, the owner, the engine or wf-agents; for between, the projects. waiting: it waits for the owner
  -- (an ask not answered, a design card not ruled, a fault not read). pending: the owner's words not taken up yet.
  -- agents: it passes between agents.
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END AS conversation,
    coalesce(w.project, i.project) AS project, 'message' AS source, m.id AS id,
    coalesce(s.line, CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect' ELSE 'engineer' END) AS sender,
    'owner' AS recipient, m.text AS text, coalesce(r.ended_at, r.started_at) AS at, m.work AS work, m.round AS round,
    NULL AS ask, NULL AS design_card, m.read_at AS read_at, 0 AS waiting, 0 AS pending, 0 AS agents, NULL AS fault
  FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
  LEFT JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
  UNION ALL
  SELECT CASE WHEN n.fault IS NOT NULL OR w.id IS NULL THEN 'wf-agents'
              WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    CASE WHEN n.fault IS NULL THEN coalesce(w.project, i.project) END, 'notice', n.id,
    CASE WHEN n.fault IS NULL THEN 'engine' ELSE 'wf-agents' END, 'owner', n.text, n.at, n.work, NULL, n.ask, NULL,
    n.read_at, n.fault IS NOT NULL AND n.read_at IS NULL, 0, 0, n.fault
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
    NULL, a.answer IS NULL, 0, 0, NULL
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'answer', a.id, 'owner',
    CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect'
         ELSE coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
                        WHERE x.work = a.work AND x.n < a.round ORDER BY x.n DESC LIMIT 1), 'engineer') END,
    coalesce(nullif(json_extract(a.answer, '$.text'), ''), json_extract(a.answer, '$.choice'), ''),
    coalesce(json_extract(a.answer, '$.at'), r.started_at), a.work, a.round, a.id, NULL, NULL, 0, 0, 0, NULL
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  WHERE a.answer IS NOT NULL AND coalesce(json_extract(a.answer, '$.by'), 'owner') <> 'engine'
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
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
