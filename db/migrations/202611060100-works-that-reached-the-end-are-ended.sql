-- Takes a database to one where a work that reached the end of its graph has `ended` = `done`, so that the engine's
-- loop no longer loads it, and whose views of the items on hand read such a work as done. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.
--
-- A work reached the end of its graph when it is at the graph's end cell, by the id any flow version gave that cell
-- (the engineer's `E1`, `20`, `end`; the architect's `R1`, `E2`, `record`), and its latest round is completed there
-- without taking a row or being sent back: the work the views' `running` leaves out. A work whose round at its end is
-- not completed is not marked: the engine takes that round up again. A paused work, as #4's work 9 is at the cell `20`
-- of flow version 3, is marked and stays paused.

DROP VIEW ready_items;
CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: first those the owner queued, in the owner's order; then the
  -- others in the architect's order, each by its rank among its project's items, lower first, those placed nowhere yet
  -- after, in the order made. An item is ready when it is not cancelled, not held, has no work running and no item it
  -- depends on by a blocking dependency unmerged (item_blocked_by), and, unless the owner queued it, is not done. An
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
  WHERE i.cancelled IS NULL AND i.held IS NULL AND i.id NOT IN (SELECT item FROM running)
    AND i.id NOT IN (SELECT item FROM item_blocked_by)
    AND (q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

DROP VIEW open_items;
CREATE VIEW open_items AS
  -- The items not done or cancelled, a short row each, in the order they start. An item's state: `running`, a work is
  -- on it, whose number and cell are in work and at; `paused`, that work is paused; `resuming`, it is paused and asked
  -- to go on, which waits for a free slot; `stopping`, a cancel or a start over was asked, and it reads as it stands
  -- until the work's step has ended; `held`, the architect holds it (items.held says why); `blocked`, an item it waits
  -- for is not merged (item_blocked_by); `ready`, it starts when a slot is free (ready_items). version is what a change
  -- to the item is written on. Read by: the agents, to see what is on hand.
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
      WHEN i.held IS NOT NULL THEN 'held'
      WHEN i.id IN (SELECT item FROM item_blocked_by) THEN 'blocked'
      ELSE 'ready'
    END AS state,
    i.rank,
    (SELECT coalesce(max(c.id), 0) FROM changes c WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id))
      AS version,
    r.work, r.at
  FROM items i LEFT JOIN running r ON r.item = i.id LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND (r.item IS NOT NULL OR q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

-- The tracking triggers come back after every migration; a migration changes rows outside the engine's own writes.
DROP TRIGGER IF EXISTS track_works_update;
UPDATE works SET ended = 'done'
WHERE ended IS NULL
  AND ((graph = 'engineer' AND at IN ('E1', '20', 'end')) OR (graph = 'architect' AND at IN ('R1', 'E2', 'record')))
  AND EXISTS (
    SELECT 1 FROM rounds r
    WHERE r.work = works.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = works.id)
      AND r.cell = works.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL);
