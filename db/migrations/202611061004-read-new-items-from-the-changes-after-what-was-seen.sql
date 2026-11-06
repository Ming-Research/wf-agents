-- Takes a database to one that reads whether an item is new to the owner from the changes after what the owner saw,
-- not from the item's whole history: owner_seen keeps the item's latest change of any kind, which every row already
-- there is brought up to, the changes of content are the view item_content_changes, and item_content_version is read
-- from it. Each statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW item_content_version;

CREATE VIEW item_content_changes AS
  -- The changes of the items' content: the rows of changes that made an item, or that changed what its creator writes
  -- (title, problem, why, approach, wanted, decision_points), by their id and the item's key. A change to any other
  -- column (the level, deliver, cancelled, and what the engine and the architect set) is no change of content.
  -- Read for one item through row_key = json_object('id', item), which finds its rows by the index of changes by row.
  -- Hidden from agents.
  -- Read by: item_content_version; the owner's interface, which marks an item new while one of these came after its
  -- owner_seen row.
  SELECT c.id, c.row_key FROM changes c
  WHERE c.table_name = 'items' AND c.new_row IS NOT NULL
    AND (c.old_row IS NULL
      OR json_extract(c.old_row, '$.title') IS NOT json_extract(c.new_row, '$.title')
      OR json_extract(c.old_row, '$.problem') IS NOT json_extract(c.new_row, '$.problem')
      OR json_extract(c.old_row, '$.why') IS NOT json_extract(c.new_row, '$.why')
      OR json_extract(c.old_row, '$.approach') IS NOT json_extract(c.new_row, '$.approach')
      OR json_extract(c.old_row, '$.wanted') IS NOT json_extract(c.new_row, '$.wanted')
      OR json_extract(c.old_row, '$.decision_points') IS NOT json_extract(c.new_row, '$.decision_points'));

CREATE VIEW item_content_version AS
  -- Each item with the change of its content that came last (item_content_changes), 0 for an item with none recorded.
  -- It reads the item's whole history, so it is for one item at a time.
  -- Hidden from agents.
  -- Read by: people and tests looking at an item's content version.
  SELECT i.id AS item, coalesce((
    SELECT max(v.id) FROM item_content_changes v WHERE v.row_key = json_object('id', i.id)), 0) AS change
  FROM items i;

UPDATE owner_seen SET change = coalesce((
  SELECT max(c.id) FROM changes c WHERE c.table_name = 'items' AND c.row_key = json_object('id', owner_seen.item)), 0);
