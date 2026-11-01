-- Takes a version 62 database to version 63: a design card becomes a decision card, one question put to the owner in
-- the shape of the reasoning system's card extension, ruled on its own: it has a key that stays the same for the same
-- question in later versions, so that rulings follow it; the records it revises as the branch had them when it was
-- posted; and it may be withdrawn, saying why. One card of a work of wf-agents' own repository whose branch changes the
-- flow carries the flow drawn from the branch, kept in the new table flow_snapshots, with the flow file it was compiled
-- from. The columns go among the others, so the table is made afresh with its rows, each card of before keyed
-- card-<id>, and the views over it with them. The owner may ask about a card in the work's group: the words to the
-- engineer, and the answer to the owner, name the card they are about (design_card in interjections and
-- owner_messages). The design's coherence review judges each card before it is posted, so the architect no longer
-- looks at one: its looks still queued are withdrawn, saying why. Each statement below that defines something is
-- written exactly as db/schema.sql writes it.

DROP VIEW conversation_entries;
DROP VIEW waiting_interjections;
DROP VIEW work_design_card;
DROP VIEW parent_design_cards;
DROP VIEW item_design_cards;

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

CREATE TABLE design_cards_version_62 AS SELECT * FROM design_cards;
DROP TABLE design_cards;

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

INSERT INTO design_cards (id, work, round, key, body, status, rulings, replaced_by, changed)
SELECT id, work, round, 'card-' || id, body, status, rulings, replaced_by, changed
FROM design_cards_version_62;
DROP TABLE design_cards_version_62;

-- The decision cards a round handed in.
CREATE INDEX design_cards_by_round ON design_cards (work, round);
-- A work's current card of each key, at most one.
CREATE UNIQUE INDEX design_cards_current ON design_cards (work, key) WHERE status IN ('open', 'ruled');

ALTER TABLE interjections ADD COLUMN design_card INTEGER REFERENCES design_cards(id);
ALTER TABLE owner_messages ADD COLUMN design_card INTEGER REFERENCES design_cards(id);

UPDATE architect_matters
SET withdrawn = 'the architect no longer looks at decision cards: the design''s coherence review judges each before it is posted'
WHERE kind = 'design_card' AND handled_round IS NULL AND withdrawn IS NULL;

CREATE VIEW item_design_cards AS
  -- Every decision card of the item's works, by key, with its rulings, and why one withdrawn was. Read by: cells `R1`,
  -- `D1` and `D4`.
  SELECT w.item, c.work, c.id AS design_card, c.round, c.key, c.body, c.status, c.rulings, c.replaced_by, c.withdrawn
  FROM design_cards c JOIN works w ON w.id = c.work;

CREATE VIEW parent_design_cards AS
  -- The current decision cards of the items that depend on this one, open or ruled: what a split-off item's parent
  -- puts to the owner. Read by: cell `D4`.
  SELECT l.depends_on AS item, l.item AS parent, c.id AS design_card, c.key, c.body, c.status, c.rulings
  FROM item_links l
  JOIN works w ON w.item = l.item
  JOIN design_cards c ON c.work = w.id
  WHERE c.status IN ('open', 'ruled');

CREATE VIEW work_design_cards AS
  -- Each work's decision cards as they stand: its current card of each key, open or ruled, with its rulings, the
  -- records it revises and the numbers of the earlier versions of its key it replaced. Read by: cells `T4` and `T5`.
  SELECT c.work, c.id AS design_card, c.round, c.key, c.body, c.revision, c.status, c.rulings,
    (SELECT json_group_array(o.id) FROM design_cards o WHERE o.work = c.work AND o.key = c.key AND o.id < c.id)
      AS replaces
  FROM design_cards c
  WHERE c.status IN ('open', 'ruled');

CREATE VIEW waiting_interjections AS
  -- Interjections not yet handed in, in the order they came, each with the decision card it asks about, by its number
  -- and key, if it does. Read by: every agent cell; cells `T7`, `M1` and `E1`.
  SELECT j.work, j.id AS interjection, j.line, j.speaker, j.text, j.design_card, c.key AS card_key
  FROM interjections j LEFT JOIN design_cards c ON c.id = j.design_card
  WHERE j.delivered_round IS NULL;

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
  -- matter the entry is, or holds the owner's words of, or whose talk it is an entry of. design_card: the decision
  -- card the entry is, or is a ruling on, or the owner's question asks about, or an answer answers about. waiting: it waits for the
  -- owner (an ask that stands for an open blocker the owner holds, or an architect's ask that holds a matter still
  -- open, neither handled nor withdrawn, not answered; a design card not ruled while its work goes on; a fault not
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
      OR EXISTS (SELECT 1 FROM blockers b WHERE b.ask = a.id AND b.ended IS NULL AND b.holder = 'owner')), 0, 0, 0, NULL
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
    j.work, j.delivered_round, NULL, j.design_card, j.matter, NULL, 0,
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
