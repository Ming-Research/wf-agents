-- Takes a database to one whose view of an item's acceptance says where its wanted conditions are before its work. The
-- statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW item_acceptance;
CREATE VIEW item_acceptance AS
  -- The item's acceptance, and the round that gave it, by its work and its number in the work; NULL until the work
  -- goes on to implement: until then what its creator wanted is `wanted` of item_body. locked says only whether there
  -- is one. Read by: cells `I1`, `T2`, `T3`, `T1`, `T6`, `V1` and
  -- `V2`.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;
