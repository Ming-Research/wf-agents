-- Takes a version 25 database to version 26: the owner's conversations, by subject, gathered from the tables that hold
-- what was said into one view the owner's interface reads. Each statement below that defines something is written
-- exactly as db/schema.sql writes it.

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
