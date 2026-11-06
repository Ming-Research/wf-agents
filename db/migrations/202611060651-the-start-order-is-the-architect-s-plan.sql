-- Takes a database to one where what goes first and what is held back for now is the architect's plan, not the item's:
-- each item's place in its project's start order and whether the plan holds it back move from items (rank, held) to
-- the new table start_order; the owner's queue (dispatch_queue) goes, its items put first in their projects, in its
-- order; and the views that read them, ready_items and open_items, are made afresh, open_items with no state of the
-- item beyond its being open. Why an item was held stays where it was written: in the items' history (changes) and in
-- the reason of the change that held it (architect_item_changes); the architect keeps it in its notebook from now on.
-- items is made afresh with its rows, so that its definition is the one db/schema.sql writes. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.

DROP VIEW open_items;
DROP VIEW ready_items;

CREATE TABLE start_order (
  -- The architect's plan of starts: where an item stands in its project's start order, the order works start in, and
  -- whether the plan holds it back from starting for now. It is the architect's plan, not the item's: the architect
  -- writes it with its changes (a move first, last, or before or after another item of its project, which ranks the
  -- project's items afresh; a hold; a release), and keeps why in its notebook, the change that made it giving its
  -- reason too (architect_item_changes). An item with no row, as one just made, is placed nowhere and not held: it
  -- starts after those placed, in the order made. Kept apart from items, a reordering writes these small rows only;
  -- kept on the items, each one rewrote every item of the project.
  -- Read by: ready_items, which cell `S0` starts from; open_items; the engine's checks of the architect's item changes;
  -- the architect.
  item INTEGER PRIMARY KEY REFERENCES items(id),
  -- Its place among its project's items, lower first, 1 the first; NULL for an item held and placed nowhere.
  place INTEGER,
  -- 1 while the plan holds it back, so that no work starts on it; 0 when it may start.
  held INTEGER NOT NULL DEFAULT 0 CHECK (held IN (0, 1)),
  CHECK (place IS NOT NULL OR held = 1)
) STRICT;

-- The owner's queue first in each project, in its order, then the architect's ranks; each project's places from 1.
INSERT INTO start_order (item, place, held)
SELECT i.id, row_number() OVER (PARTITION BY i.project ORDER BY q.position IS NULL, q.position, i.rank, i.id),
  i.held IS NOT NULL
FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id
WHERE i.rank IS NOT NULL OR q.item IS NOT NULL;

-- An item held and placed nowhere stays held, placed nowhere.
INSERT INTO start_order (item, place, held)
SELECT id, NULL, 1 FROM items
WHERE held IS NOT NULL AND rank IS NULL AND id NOT IN (SELECT item FROM dispatch_queue);

DROP TABLE dispatch_queue;

CREATE TABLE items_before_start_order AS SELECT * FROM items;
DROP TABLE items;

CREATE TABLE items (
  -- A piece of work to do, as its creator wrote it. Text that a work's rounds produce later (the problem as the
  -- engineer understood it, the study, the design, progress) is not stored here: views read it from rounds. Items own
  -- nothing; how they depend on each other is in item_links. An item is open, done or cancelled: only cancellation is
  -- stored; it is done once a work of it reached the end of its graph. What the architect's plan does with it, which
  -- comes first and which is held back, is the plan's (start_order), not the item's. A goal is an item with no work of
  -- its own, which no work ever runs on: it is reached through the items it depends on, and is done once each of them
  -- is (goals_open).
  -- Read by: every cell that reads the item or the items around it; the engine when it starts and stops works.

  -- Item number, one sequence across all projects.
  id INTEGER PRIMARY KEY,
  -- The project the item belongs to. Picks the repository, CI and settings its works run with.
  -- Read by: the engine; cells `R1` and `D1`, through the project's design goals.
  project TEXT NOT NULL,
  -- What to do, as an action. Read by: cell `R1` for the items around this one; people.
  title TEXT NOT NULL,
  -- What cannot be done or is wrong, and why the work is worth doing, as written at creation. The engineer's own
  -- understanding replaces them in views once cell `R1` has run; these stay as written.
  -- Read by: cell `R1`, for this item and the items that depend on it.
  problem TEXT NOT NULL,
  why TEXT NOT NULL,
  -- How to do it, when the creator already knows; NULL otherwise. Read by: cell `R1`.
  approach TEXT,
  -- What the work hands in: a change to merge, or a report; NULL for a goal, which has no work. Read by: cells `R1`,
  -- `D1` and `I1`; the engine.
  deliver TEXT CHECK (deliver IS NULL OR deliver IN ('change', 'report')),
  -- Agent level; decides which agent and model run the work; NULL for a goal. Read by: the engine at cell `S1`.
  level TEXT,
  -- The acceptance: a JSON list of acceptance rows in the flow's acceptance shape, as the research's plan or the design
  -- the owner approved gave them when the work went on to implement, and as the implementation, a fix or the advisor
  -- changed them since, each saying why; NULL until then. Read by: cells `I1`, `T2`, `T3`, `T1` and `T6`; the engine,
  -- which checks the conditions met against it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round that gave the acceptance, by its work and its number in the work: the research's, the design's, or the
  -- round of the implementation, a fix or the advisor that changed it. Read by: people following a condition back.
  acceptance_work INTEGER,
  acceptance_round INTEGER,
  -- Why the item was cancelled; NULL while it is not. Read by: the engine; cell `R1` for the items around
  -- this one.
  cancelled TEXT,
  -- For an item imported from outside this flow, such as an item of an earlier board: where it came from, as the import
  -- wrote it; NULL for an item made here. Read by: cell `R1`, through what already exists for the item;
  -- the owner's interface.
  imported_from TEXT,
  -- The branch and the pull request that work on the item had outside this flow, as the import recorded them; NULL when
  -- there were none. Read by: cell `S1`, which takes them over when the item had no work here before; cell
  -- `R1`; the owner's interface.
  imported_branch TEXT,
  imported_pr INTEGER,
  -- The acceptance as the creator wrote it, JSON list of conditions; NULL when the creator wrote none. The acceptance
  -- the work goes by is acceptance. Read by: cell `R1`, through item_body.
  wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted)),
  -- The decision points the item carries, JSON list in the flow's decision point shape, each with the option the owner
  -- chose and why: what a discussion settled that goes into the project's decisions/ once deep research confirms it.
  -- NULL when it carries none. An item that carries one goes to deep research, whatever its research finds. The
  -- architect writes them as it creates or edits the item. Read by: the engine's value `decision_points`; cells `R1`,
  -- `D1` and `T2`, through item_body.
  decision_points TEXT CHECK (decision_points IS NULL OR json_valid(decision_points)),
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  CHECK ((deliver IS NULL) = (level IS NULL)),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO items (
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, decision_points)
SELECT
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, decision_points
FROM items_before_start_order;
DROP TABLE items_before_start_order;

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: first each item whose latest work was taken over (started over by
  -- the owner or its architect, or one its flow version could not carry on), which waits only for a free slot, the one
  -- taken over first first; then the others in the architect's start order (start_order), each by its place among its
  -- project's items, lower first, those placed nowhere after, in the order made. An item is ready when it is no goal,
  -- not cancelled, not done, not held by the plan, has no work running and no item it depends on by a blocking
  -- dependency unmerged (item_blocked_by). On an item not cancelled, a work ends before its end only when it is taken
  -- over. A work runs until it ends or its last round, at the cell it is at, completed without taking a row, which only
  -- the end of its graph does; it is done at the end of the engineer's graph, ended as `done` or, while its round there
  -- is not completed, not ended. While dispatch_holds has a row for its project, the engine starts none.
  -- Read by: cell `S0`; the architect, through what runs and starts next.
  WITH
  running(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND (w.ended IS NULL OR w.ended = 'done') AND w.graph = 'engineer'
      AND w.at IN ('E1', '20', 'end')),
  taken_over(item, work) AS (
    SELECT w.item, w.id FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NOT NULL AND w.ended <> 'done'
      AND w.id = (SELECT max(x.id) FROM works x WHERE x.item = w.item))
  SELECT i.id AS item, i.project, i.title, i.level, s.place, t.work IS NOT NULL AS taken_over
  FROM items i LEFT JOIN start_order s ON s.item = i.id LEFT JOIN taken_over t ON t.item = i.id
  WHERE i.deliver IS NOT NULL AND i.cancelled IS NULL AND coalesce(s.held, 0) = 0
    AND i.id NOT IN (SELECT item FROM running) AND i.id NOT IN (SELECT item FROM item_blocked_by)
    AND i.id NOT IN (SELECT item FROM done)
  ORDER BY t.work IS NULL, t.work, s.place NULLS LAST, i.id;

CREATE VIEW open_items AS
  -- The items not done and not cancelled, a short row each, in the architect's start order. An item is open, done or
  -- cancelled, and these are the open ones: what its work does, what the architect's plan does with it and what it
  -- waits for are columns of their own. goal: 1 for a goal, which has no work of its own and is done once each item it
  -- depends on is (goals_open). work and at: the work going on on it and the cell it is at; NULL while none is.
  -- work_state, what that work does now: `stopping`, a cancel or a start over was asked, and it stands as it is until
  -- the step it is in has ended; `paused`, by the owner, its architect or the engine (works.paused says why);
  -- `waiting_for_slot`, its pause was asked to be lifted, or what it waited for came, and it waits for a free slot;
  -- `waiting`, it waits for the holder of its blocker (blockers: the owner, an architect, its advisor, or items to
  -- deliver); `running` otherwise: a round of it runs, its next step is due, or it waits at its cell for its turn or to
  -- run a failed round again. held: 1 while the architect's plan holds it back (start_order). blocked: 1 while an item
  -- it depends on by a blocking dependency is not merged (item_blocked_by), which keeps it from starting; a work going
  -- on goes on either way. place: its place in its project's start order, NULL for one placed nowhere. version is what
  -- a change to the item is written on. Read by: the agents, to see what is on hand.
  WITH
  running(item, work, at, paused) AS (
    SELECT w.item, w.id, w.at, w.paused FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.at NOT IN ('E1', '20', 'end') AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND (w.ended IS NULL OR w.ended = 'done') AND w.graph = 'engineer'
      AND w.at IN ('E1', '20', 'end'))
  SELECT i.id AS item, i.project, i.title, i.deliver IS NULL AS goal,
    CASE
      WHEN r.work IS NULL THEN NULL
      WHEN EXISTS (
        SELECT 1 FROM outbox o
        WHERE (o.kind = 'start_over' AND o.work = r.work)
          OR (o.kind = 'cancel_item' AND json_extract(o.args, '$.item') = i.id)) THEN 'stopping'
      WHEN r.paused IS NOT NULL AND EXISTS (
        SELECT 1 FROM outbox o WHERE o.kind = 'resume_work' AND o.work = r.work) THEN 'waiting_for_slot'
      WHEN r.paused IS NOT NULL THEN 'paused'
      WHEN EXISTS (SELECT 1 FROM slot_waits w WHERE w.work = r.work AND w.until IS NULL) THEN 'waiting_for_slot'
      WHEN EXISTS (SELECT 1 FROM blockers b WHERE b.work = r.work AND b.ended IS NULL) THEN 'waiting'
      ELSE 'running'
    END AS work_state,
    coalesce(s.held, 0) AS held,
    i.id IN (SELECT item FROM item_blocked_by) AS blocked,
    s.place,
    (SELECT coalesce(max(c.id), 0) FROM changes c WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id))
      AS version,
    r.work, r.at
  FROM items i LEFT JOIN running r ON r.item = i.id LEFT JOIN start_order s ON s.item = i.id
  WHERE i.cancelled IS NULL AND CASE
    WHEN i.deliver IS NULL THEN i.id IN (SELECT item FROM goals_open)
    ELSE i.id NOT IN (SELECT item FROM done)
  END
  ORDER BY s.place NULLS LAST, i.id;
