-- Takes a database to one that keeps which version of each item the owner has seen, so that the owner's page marks the
-- items that are new to them: the table owner_seen, the view item_content_version that gives each item's latest
-- change of content, and a row for each item already there at its current content version, so that nothing is new at
-- the start. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE owner_seen (
  -- Which version of each item the owner has seen: the last content change of the item (item_content_version) that
  -- the owner had seen when they last opened the item. An item is new to the owner when it has no row here, or its
  -- content version is newer than the row's change; it is new when it is created and again each time its content
  -- changes, and opening it marks it seen. Kept apart from items, which hold what its creator wrote and not what the
  -- owner has read.
  -- Hidden from agents.
  -- Read by: the owner's interface, which marks each item new to the owner.
  -- The item.
  item INTEGER PRIMARY KEY REFERENCES items(id),
  -- The id in changes of the item's latest content change that the owner had seen.
  change INTEGER NOT NULL
) STRICT;

CREATE VIEW item_content_version AS
  -- Each item with the change of its content that came last: the id in changes of the row that made the item, or that
  -- changed what its creator writes (title, problem, why, approach, wanted, decision_points). A change to any other
  -- column (the level, deliver, cancelled, and what the engine and the architect set) is no change of content. 0 for
  -- an item with no change recorded.
  -- Hidden from agents.
  -- Read by: the owner's interface, which marks an item new while this is newer than its owner_seen row.
  SELECT i.id AS item, coalesce((
    SELECT max(c.id) FROM changes c
    WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id) AND c.new_row IS NOT NULL
      AND (c.old_row IS NULL
        OR json_extract(c.old_row, '$.title') IS NOT json_extract(c.new_row, '$.title')
        OR json_extract(c.old_row, '$.problem') IS NOT json_extract(c.new_row, '$.problem')
        OR json_extract(c.old_row, '$.why') IS NOT json_extract(c.new_row, '$.why')
        OR json_extract(c.old_row, '$.approach') IS NOT json_extract(c.new_row, '$.approach')
        OR json_extract(c.old_row, '$.wanted') IS NOT json_extract(c.new_row, '$.wanted')
        OR json_extract(c.old_row, '$.decision_points') IS NOT json_extract(c.new_row, '$.decision_points'))), 0)
    AS change
  FROM items i;

INSERT INTO owner_seen (item, change) SELECT item, change FROM item_content_version;
