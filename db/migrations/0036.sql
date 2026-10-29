-- Takes a version 35 database to version 36: what a stopped task waits on is one record, a blocker, with one holder
-- at a time, instead of a record of its own for each kind: an ask of the owner, an engineer's question or failure
-- with the architect's reply on its matter, the wait for items at `A6` and the merge gate's wait at `M1`. Each record
-- of those kinds of an engineer's work becomes a blocker: those still open as the open blocker of their work, the
-- others with how they ended. Each statement below that defines something is written exactly as db/schema.sql writes
-- it.

CREATE TABLE blockers (
  -- What a stopped task waits on, one row for each stop: an engineer's question, an engine tool's failure, an ask of the
  -- owner, a wait for items to deliver, or a wait for the owner's ruling on a design card. A task is an engineer's work;
  -- a wake of the architect is never one. A blocker belongs to the task and the cell that raised it, and has exactly one
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

CREATE UNIQUE INDEX blockers_open ON blockers (work) WHERE ended IS NULL;

-- The asks of the owner of engineers' works: an ask the owner answered for the architect, as an earlier engine put
-- some, was a question; an ask the engine closed because its work ended was withdrawn. The cell and round that raised
-- it are the work's last round before it outside the asking part.
INSERT INTO blockers (work, cell, round, kind, what, holder, ask, at, since, ended, outcome, ended_at)
SELECT a.work,
  coalesce((SELECT x.cell FROM rounds x WHERE x.work = a.work AND x.n < a.round
              AND x.cell NOT IN ('A1', '12', 'advisor', 'ask.advisor', 'A2', '15', 'to_architect', 'ask.architect',
                'A3', '16', 'to_owner', 'ask.owner')
            ORDER BY x.n DESC LIMIT 1), r.cell),
  (SELECT x.n FROM rounds x WHERE x.work = a.work AND x.n < a.round
     AND x.cell NOT IN ('A1', '12', 'advisor', 'ask.advisor', 'A2', '15', 'to_architect', 'ask.architect',
       'A3', '16', 'to_owner', 'ask.owner')
   ORDER BY x.n DESC LIMIT 1),
  CASE WHEN json_extract(a.body, '$.to') = 'architect' THEN 'question' ELSE 'owner_ask' END,
  coalesce((SELECT n.text FROM engine_notices n WHERE n.ask = a.id ORDER BY n.id LIMIT 1),
    'Work ' || a.work || ' asks the owner'),
  'owner', a.id, r.started_at, r.started_at,
  CASE WHEN a.answer IS NULL THEN NULL
       WHEN json_extract(a.answer, '$.by') = 'engine' THEN 'withdrawn' ELSE 'resolved' END,
  CASE WHEN a.answer IS NULL THEN NULL
       WHEN json_extract(a.answer, '$.by') = 'engine' THEN json_object('why', json_extract(a.answer, '$.text'))
       ELSE a.answer END,
  CASE WHEN a.answer IS NULL THEN NULL
       ELSE coalesce(json_extract(a.answer, '$.at'), r.ended_at, r.started_at) END
FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
WHERE w.graph = 'engineer'
  AND (a.answer IS NOT NULL OR w.ended IS NOT NULL OR NOT EXISTS (
    SELECT 1 FROM owner_asks b WHERE b.work = a.work AND b.answer IS NULL AND b.id > a.id))
ORDER BY a.id;

-- An ask still open of a work that ended needs no answer: it is withdrawn.
UPDATE blockers SET ended = 'withdrawn', outcome = json_object('why', 'the work ended'),
  ended_at = coalesce((SELECT max(r.ended_at) FROM rounds r WHERE r.work = blockers.work), blockers.at)
WHERE ended IS NULL AND EXISTS (SELECT 1 FROM works w WHERE w.id = blockers.work AND w.ended IS NOT NULL);

-- The engineers' questions and failures the architect was asked: resolved by its reply, withdrawn when their work
-- stopped waiting or ended, open while neither, with a reply that waits for its batch said about it.
INSERT INTO blockers (work, cell, round, kind, what, holder, project, matter, at, since, said, ended, outcome,
  ended_at)
SELECT m.work,
  coalesce((SELECT x.cell FROM rounds x WHERE x.work = m.work AND x.n < m.round
              AND x.cell NOT IN ('A1', '12', 'advisor', 'ask.advisor', 'A2', '15', 'to_architect', 'ask.architect',
                'A3', '16', 'to_owner', 'ask.owner')
            ORDER BY x.n DESC LIMIT 1), r.cell),
  (SELECT x.n FROM rounds x WHERE x.work = m.work AND x.n < m.round
     AND x.cell NOT IN ('A1', '12', 'advisor', 'ask.advisor', 'A2', '15', 'to_architect', 'ask.architect',
       'A3', '16', 'to_owner', 'ask.owner')
   ORDER BY x.n DESC LIMIT 1),
  CASE WHEN m.kind = 'failure' THEN 'tool_failure' ELSE 'question' END,
  m.what, 'architect', m.project, m.id, m.at, m.at,
  CASE WHEN m.reply IS NULL AND m.reply_held IS NOT NULL
       THEN json_array(json_object('at', m.at, 'by', 'architect', 'kind', 'held_reply', 'what', json(m.reply_held)))
       ELSE '[]' END,
  CASE WHEN m.reply IS NOT NULL THEN 'resolved' WHEN m.withdrawn IS NOT NULL OR w.ended IS NOT NULL THEN 'withdrawn'
  END,
  CASE WHEN m.reply IS NOT NULL THEN m.reply
       WHEN m.withdrawn IS NOT NULL THEN json_object('why', m.withdrawn)
       WHEN w.ended IS NOT NULL THEN json_object('why', 'the work ended') END,
  CASE WHEN m.reply IS NOT NULL
         THEN coalesce((SELECT x.ended_at FROM rounds x WHERE x.work = m.handled_work AND x.n = m.handled_round), m.at)
       WHEN m.withdrawn IS NOT NULL OR w.ended IS NOT NULL
         THEN coalesce((SELECT max(x.ended_at) FROM rounds x WHERE x.work = m.work), m.at) END
FROM architect_matters m JOIN works w ON w.id = m.work JOIN rounds r ON r.work = m.work AND r.n = m.round
WHERE w.graph = 'engineer' AND m.kind IN ('engineer_question', 'failure')
  AND (m.reply IS NOT NULL OR m.withdrawn IS NOT NULL OR w.ended IS NOT NULL
    OR NOT EXISTS (SELECT 1 FROM blockers b WHERE b.work = m.work AND b.ended IS NULL))
ORDER BY m.id;

-- A work that waits at `A6` for items to deliver: held by the items it depends on not merged yet, each with its
-- project.
INSERT INTO blockers (work, cell, kind, what, holder, items, at, since)
SELECT w.id, w.at, 'items', 'Work ' || w.id || ' waits for the items it depends on to deliver', 'items',
  (SELECT json_group_array(json_object('item', d.id, 'project', d.project))
   FROM item_links l JOIN items d ON d.id = l.depends_on
   WHERE l.item = w.item AND d.cancelled IS NULL AND NOT EXISTS (
     SELECT 1 FROM rounds x JOIN works y ON y.id = x.work
     WHERE y.item = d.id AND x.cell IN ('M2', '9', 'merge_queue', 'merge.queue') AND x.transition = 'merged')),
  coalesce((SELECT max(x.ended_at) FROM rounds x WHERE x.work = w.id), 0),
  coalesce((SELECT max(x.ended_at) FROM rounds x WHERE x.work = w.id), 0)
FROM works w
WHERE w.graph = 'engineer' AND w.ended IS NULL AND w.at IN ('A6', '11b', 'wait_items', 'ask.wait_items')
  AND NOT EXISTS (SELECT 1 FROM blockers b WHERE b.work = w.id AND b.ended IS NULL);

-- A work that waits at the merge gate (`M1`): held by the items it depends on while one is not merged, each with its
-- project; by the owner, for the ruling on its design card, once every one is.
INSERT INTO blockers (work, cell, round, kind, what, holder, items, at, since)
SELECT w.id, w.at, r.n,
  CASE WHEN u.items IS NULL THEN 'ruling' ELSE 'items' END,
  CASE WHEN u.items IS NULL THEN 'Work ' || w.id || ' waits for the owner''s ruling on its design card to merge'
       ELSE 'Work ' || w.id || ' waits for the items it depends on to merge' END,
  CASE WHEN u.items IS NULL THEN 'owner' ELSE 'items' END,
  u.items, r.started_at, r.started_at
FROM works w
JOIN rounds r ON r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
JOIN (SELECT y.id AS work,
        (SELECT json_group_array(json_object('item', d.id, 'project', d.project))
         FROM item_links l JOIN items d ON d.id = l.depends_on
         WHERE l.item = y.item AND d.cancelled IS NULL AND NOT EXISTS (
           SELECT 1 FROM rounds x JOIN works z ON z.id = x.work
           WHERE z.item = d.id AND x.cell IN ('M2', '9', 'merge_queue', 'merge.queue') AND x.transition = 'merged')
         HAVING count(*) > 0) AS items
      FROM works y) u ON u.work = w.id
WHERE w.graph = 'engineer' AND w.ended IS NULL AND w.at IN ('M1', '19', 'merge_gate', 'merge.gate')
  AND r.cell = w.at AND r.transition = 'wait'
  AND NOT EXISTS (SELECT 1 FROM blockers b WHERE b.work = w.id AND b.ended IS NULL);
