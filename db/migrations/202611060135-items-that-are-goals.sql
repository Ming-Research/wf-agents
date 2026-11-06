-- Takes a database to one whose items may be goals: an item with no work of its own, reached through the items it
-- depends on, has no deliver and no level. Their table is made afresh with its rows, as SQLite cannot drop a NOT NULL
-- constraint; the views that tell whether an item is ready, open or held back by another learn of goals through the
-- new view goals_open. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE items_before_goals AS SELECT * FROM items;
DROP TABLE items;

CREATE TABLE items (
  -- A piece of work to do, as its creator wrote it. Text that a work's rounds produce later (the problem as the
  -- engineer understood it, the study, the design, progress) is not stored here: views read it from rounds. Items own
  -- nothing; how they depend on each other is in item_links. Only cancellation is stored as a status; every other
  -- status is derived. A goal is an item with no work of its own, which no work ever runs on: it is reached through
  -- the items it depends on, and is done once each of them is (goals_open).
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
  -- Why the architect holds the item, with the owner's approval: no work starts on it while it is held; NULL while it
  -- is not. Read by: ready_items; the architect.
  held TEXT,
  -- Its place among its project's items in the architect's order, which works start in, lower first: the architect
  -- sets it with its approved changes, moving an item first, last, or before or after another item of the project,
  -- which ranks the project's items afresh. NULL for an item placed nowhere yet, as one just made is, which comes after
  -- those placed, in the order made. Read by: the dispatch order in ready_items; the architect.
  rank INTEGER,
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
  imported_from, imported_branch, imported_pr, wanted, held, rank, decision_points)
SELECT
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, held, rank, decision_points
FROM items_before_goals;
DROP TABLE items_before_goals;

CREATE VIEW goals_open AS
  -- The goals not done, each with its project: a goal is done once every item it depends on is, a cancelled one left
  -- out, and one that depends on nothing is not done. An item with a work of its own is done when a work of it reached
  -- the end of the engineer's graph, ended as `done` or, while its round there is not completed, not ended.
  -- Read by: open_items; item_blocked_by; the engine, where a work waits for the items it depends on; the owner's
  -- interface.
  WITH RECURSIVE
  done(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND (w.ended IS NULL OR w.ended = 'done') AND w.graph = 'engineer'
      AND w.at IN ('E1', '20', 'end')),
  unfinished(item) AS (
    SELECT i.id FROM items i
    WHERE i.deliver IS NOT NULL AND i.cancelled IS NULL AND i.id NOT IN (SELECT item FROM done)
    UNION
    SELECT g.id FROM items g
    WHERE g.deliver IS NULL AND g.cancelled IS NULL AND NOT EXISTS (
      SELECT 1 FROM item_links l JOIN items d ON d.id = l.depends_on WHERE l.item = g.id AND d.cancelled IS NULL)
    UNION
    SELECT g.id FROM item_links l JOIN unfinished o ON o.item = l.depends_on JOIN items g ON g.id = l.item
    WHERE g.deliver IS NULL AND g.cancelled IS NULL)
  SELECT g.id AS item, g.project FROM items g
  WHERE g.deliver IS NULL AND g.cancelled IS NULL AND g.id IN (SELECT item FROM unfinished);

DROP VIEW item_blocked_by;

CREATE VIEW item_blocked_by AS
  -- Each item with each item it depends on by a blocking dependency that is not merged yet, as the merge cell
  -- (`M2`) records a merge, or, for a goal, not done yet (goals_open): no work starts on the item until every one is. A
  -- cancelled item, which never delivers, holds back none. Read by: ready_items; the agents; the owner's interface,
  -- which shows an item that would start but for them as waiting for them.
  SELECT l.item, l.depends_on, l.needs
  FROM item_links l JOIN items d ON d.id = l.depends_on
  WHERE l.kind = 'blocking' AND d.cancelled IS NULL AND CASE
    WHEN d.deliver IS NULL THEN d.id IN (SELECT item FROM goals_open)
    ELSE NOT EXISTS (
      SELECT 1 FROM rounds r JOIN works w ON w.id = r.work
      WHERE w.item = l.depends_on AND r.cell IN ('M2', '9', 'merge_queue', 'merge.queue') AND r.transition = 'merged')
  END;

DROP VIEW ready_items;

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: first those the owner queued, in the owner's order; then the
  -- others in the architect's order, each by its rank among its project's items, lower first, those placed nowhere yet
  -- after, in the order made. An item is ready when it is no goal, not cancelled, not held, has no work running and no
  -- item it depends on by a blocking dependency unmerged (item_blocked_by), and, unless the owner queued it, is not
  -- done. An
  -- item leaves the owner's queue when a work starts on it, and may be queued again once its works ended. A work runs
  -- until it ends or its last round, at the cell it is at, completed without taking a row, which only the end of its
  -- graph does; it is done at the end of the engineer's graph, ended as `done` or, while its round there is not
  -- completed, not ended. While dispatch_holds has a row, the engine starts none.
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
      AND w.at IN ('E1', '20', 'end'))
  SELECT i.id AS item, i.project, i.title, i.level, q.position, i.rank
  FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.deliver IS NOT NULL AND i.cancelled IS NULL AND i.held IS NULL AND i.id NOT IN (SELECT item FROM running)
    AND i.id NOT IN (SELECT item FROM item_blocked_by)
    AND (q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

DROP VIEW open_items;

CREATE VIEW open_items AS
  -- The items not done or cancelled, a short row each, in the order they start. An item's state: `running`, a work is
  -- on it, whose number and cell are in work and at; `paused`, that work is paused; `resuming`, it is paused and asked
  -- to go on, which waits for a free slot; `stopping`, a cancel or a start over was asked, and it reads as it stands
  -- until the work's step has ended; `held`, the architect holds it (items.held says why); `blocked`, an item it waits
  -- for is not merged (item_blocked_by); `ready`, it starts when a slot is free (ready_items); `goal`, it has no work
  -- of its own and is not done yet (goals_open). version is what a change to the item is written on. Read by: the
  -- agents, to see what is on hand.
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
  SELECT i.id AS item, i.project, i.title,
    CASE
      WHEN EXISTS (
        SELECT 1 FROM outbox o
        WHERE (o.kind = 'start_over' AND o.work = r.work)
          OR (o.kind = 'cancel_item' AND json_extract(o.args, '$.item') = i.id)) THEN 'stopping'
      WHEN r.paused IS NOT NULL AND EXISTS (
        SELECT 1 FROM outbox o WHERE o.kind = 'resume_work' AND o.work = r.work) THEN 'resuming'
      WHEN r.paused IS NOT NULL THEN 'paused'
      WHEN r.item IS NOT NULL THEN 'running'
      WHEN i.deliver IS NULL THEN 'goal'
      WHEN i.held IS NOT NULL THEN 'held'
      WHEN i.id IN (SELECT item FROM item_blocked_by) THEN 'blocked'
      ELSE 'ready'
    END AS state,
    i.rank,
    (SELECT coalesce(max(c.id), 0) FROM changes c WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id))
      AS version,
    r.work, r.at
  FROM items i LEFT JOIN running r ON r.item = i.id LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND CASE
    WHEN i.deliver IS NULL THEN i.id IN (SELECT item FROM goals_open)
    ELSE r.item IS NOT NULL OR q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done)
  END
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;
