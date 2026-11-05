-- Takes a database to one with the view of what each acceptance condition written in_effect came to, once the work's
-- change took effect; the view of the item's acceptance says the two new cells that read it. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.

DROP VIEW item_acceptance;

CREATE VIEW item_acceptance AS
  -- The item's acceptance, and the round that gave it, by its work and its number in the work; NULL until the work
  -- goes on to implement. locked says only whether there is one. Read by: cells `I1`, `T2`, `T3`, `T1`, `T6`, `W1` and
  -- `W2`.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;

CREATE VIEW item_verifications AS
  -- What each acceptance condition written in_effect came to, at each look of the cell that checks it once the work's
  -- change took effect (`W2`): the item, the work and the round, the condition's number (`id`), its verdict, and for a
  -- verdict that rests on a run the run and what it showed there. A condition stands as its latest round has it. Read
  -- by: the agents and the architect, by query.
  SELECT w.item, r.work, r.n AS round, json_extract(v.value, '$.id') AS id,
    json_extract(v.value, '$.verdict') AS verdict,
    json_extract(v.value, '$.run') AS run,
    json_extract(v.value, '$.seen') AS seen
  FROM rounds r JOIN works w ON w.id = r.work, json_each(r.output, '$.outcome.verdicts') v
  WHERE r.cell = 'W2' AND r.transition IS NOT NULL;
