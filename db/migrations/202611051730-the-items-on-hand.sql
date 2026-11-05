-- Takes a database to one with the view of the items on hand, a short row each, that agents look up instead of being
-- given the list. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE VIEW open_items AS
  -- The items not done or cancelled, a short row each, in the order they start. An item's state: `running`, a work is
  -- on it, whose number and cell are in work and at; `paused`, that work is paused; `held`, the architect holds it
  -- (items.held says why); `blocked`, an item it waits for is not merged (item_blocked_by); `ready`, it starts when a
  -- slot is free (ready_items). Read by: the agents, to see what is on hand.
  WITH
  running(item, work, at, paused) AS (
    SELECT w.item, w.id, w.at, w.paused FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.graph = 'engineer' AND w.at IN ('E1', '20', 'end'))
  SELECT i.id AS item, i.project, i.title,
    CASE
      WHEN r.paused IS NOT NULL THEN 'paused'
      WHEN r.item IS NOT NULL THEN 'running'
      WHEN i.held IS NOT NULL THEN 'held'
      WHEN i.id IN (SELECT item FROM item_blocked_by) THEN 'blocked'
      ELSE 'ready'
    END AS state,
    i.rank, r.work, r.at
  FROM items i LEFT JOIN running r ON r.item = i.id LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND (r.item IS NOT NULL OR q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;
