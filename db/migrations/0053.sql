-- Takes a version 52 database to version 53: the owner's words to a project's architect are each a matter of its own,
-- which its architect answers in one round, in the conversation they were said in, and no talk with the owner waits
-- for the owner's reply or is held or ended by Done any longer. The architects' matters lose when a talk was held and
-- ended, so their table is made afresh with its rows, and the queue no longer holds a talk back; the conversations put
-- the architect's reply to the owner's words said in a work's group in that group. The entries of a talk stay to read
-- old talks, and nothing writes them; the entries of a talk still open that no turn of it took yet are the owner's
-- words, each a matter of its own now, as every word the owner says to the architect is. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.

INSERT INTO architect_matters (project, kind, detail, what, body, at)
SELECT m.project, 'owner_words', 'said', 'The owner says: ' || e.text, json_object('text', e.text, 'where', 'page'),
  e.at
FROM talk_entries e JOIN architect_matters m ON m.id = e.matter
WHERE e.taken_round IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL
ORDER BY e.id;
DELETE FROM talk_entries WHERE taken_round IS NULL AND matter IN (
  SELECT id FROM architect_matters WHERE handled_round IS NULL AND withdrawn IS NULL);

DROP VIEW architect_queue;
DROP VIEW conversation_entries;

CREATE TABLE architect_matters_version_52 AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, a place its plan of what to do next found to set straight, or the check of its
  -- policy or the plan of what to do next, which come at the head of its queue. The project's queue is its
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
  -- owner_words, other_architect, slots_free, or check_policy and make_plan, the check of the policy in force and the
  -- plan of what to do next, which the architect's cells queue at the head of the queue, one of each open at a time. A
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
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the approval of
  -- the draft of a new policy, the cell that wrote it, a reply of exactly approved going to where it takes effect
  -- instead.
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
  -- ended, saying how; the owner answered the question in the work's group, the reply saying what; it said that a
  -- setting of the policy, a work of its own then, had ended, when the setting became a stage of the architect's
  -- works; or it was a setting of the policy as a stage of its own, which runs so no longer. NULL otherwise.
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
SELECT id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, reply, reply_held, handled_work, handled_round, withdrawn FROM architect_matters_version_52;
DROP TABLE architect_matters_version_52;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell for its question; the owner, for
  -- the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer and to which cell, and whether it is urgent.
  -- Read by: cells `S0`, `S1` and `Q0`; the engine, which takes `S0`'s rows by it for each project whose architect
  -- runs no work, and gives the architect its queue; the owner's interface.
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
    m.resume_cell AS back_to,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;

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
  -- an entry of its own beside it. sender and recipient are a line, the owner, the engine or wf-agents, the recipient of
  -- the owner's words in a work's group the engineer, the architect or both, and in the one-to-one the line of the
  -- architect's work they were said to (architect or policy); for between, the projects. matter: the architect's
  -- matter the entry is, or holds the owner's words of, or whose talk it is an entry of. waiting: it waits for the
  -- owner (an ask that stands for an open blocker the owner holds, or an architect's ask that holds a matter, not
  -- answered; a design card not ruled while its work goes on; a fault not read). pending: the owner's words not taken
  -- up yet. undelivered: the owner's words to an agent that its work ended without taking in. answerable: an
  -- engineer's question the architect holds now, which the owner may answer in the group instead.
  SELECT CASE WHEN w.graph = 'engineer' THEN 'work:' || w.id
              WHEN r.transition = 'answers' THEN 'architect:' || w.project
              ELSE coalesce((SELECT CASE WHEN y.graph = 'engineer' THEN 'work:' || x.work
                                         WHEN x.kind = 'owner_words' AND json_extract(x.body, '$.where') = 'work'
                                           THEN 'work:' || json_extract(x.body, '$.work') END
                             FROM architect_matters x LEFT JOIN works y ON y.id = x.work
                             WHERE x.id = (
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
    coalesce(json_extract(a.body, '$.question'),
             (SELECT group_concat(json_extract(p.value, '$.what'), char(10))
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
