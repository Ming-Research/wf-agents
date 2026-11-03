-- Takes a version 68 database to version 69: the merge queue is in the order the works came to the merge cell, not in
-- the order of their numbers, so that a work that comes back to it after a fix, or a work that began later and got
-- there sooner, stands behind the works already in the queue. The order is read from the changes of each work's row, so
-- no column is added and no row is changed. Each statement below that defines something is written exactly as
-- db/schema.sql writes it.

DROP VIEW merge_queue;

CREATE VIEW merge_queue AS
  -- Each project's merge queue: the works at the merge cell, in the order they came to it; a work that left it and
  -- came back is last. Read by: cell `M2`.
  SELECT i.project, w.id AS work, w.pr, w.pushed_head
  FROM works w JOIN items i ON i.id = w.item
  WHERE w.at IN ('M2', '9', 'merge_queue', 'merge.queue') AND w.ended IS NULL
  -- The newest change of the row that changed `at` brought it to this cell. Scanned from the newest change back, so
  -- it stops after the few since then.
  ORDER BY (SELECT c.id FROM changes c
            WHERE c.table_name = 'works' AND c.row_key = json_object('id', w.id)
              AND json_extract(c.old_row, '$.at') IS NOT json_extract(c.new_row, '$.at')
            ORDER BY c.id DESC LIMIT 1), w.id;
