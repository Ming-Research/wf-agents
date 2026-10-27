-- Takes a version 5 database to version 6. An item can hold what an import recorded about work done on it outside
-- this flow: where it came from, its branch and its pull request. An item leaves the queue when a work starts on it,
-- so that one whose works ended can be queued again; the items a work started on before leave it now. Each
-- statement below that defines something is written exactly as db/schema.sql writes it.

ALTER TABLE items ADD COLUMN imported_from TEXT;
ALTER TABLE items ADD COLUMN imported_branch TEXT;
ALTER TABLE items ADD COLUMN imported_pr INTEGER;

-- The change is the migration's, not a writer's, so the trigger that records deletions from the queue is dropped
-- first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_dispatch_queue_delete;
DELETE FROM dispatch_queue WHERE item IN (SELECT item FROM works WHERE item IS NOT NULL);

DROP VIEW ready_items;

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: queued, not cancelled, and with no work running. An item leaves
  -- the queue when a work starts on it, and may be queued again once its works ended. A work runs until it ends or
  -- its last round, at the cell it is at, completed without taking a row, which only the end of its graph does.
  -- Read by: cell 0.
  SELECT i.id AS item, i.project, i.title, i.level, q.position
  FROM dispatch_queue q JOIN items i ON i.id = q.item
  WHERE i.cancelled IS NULL AND NOT EXISTS (
    SELECT 1 FROM works w
    WHERE w.item = i.id AND w.ended IS NULL AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.id = (SELECT max(x.id) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL))
  ORDER BY q.position;
