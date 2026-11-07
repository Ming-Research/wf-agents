-- Takes a database to one with no cells after the merge: no work writes what it learned, and none waits for its change
-- to take effect to check what only real runs show. The view of what each condition so checked came to, and the view of
-- what the owner said that concerns a work, which only the writing of what was learned read, go; the rounds they read
-- stay, as the history of the works that ran there. The views that named those cells among their readers are made
-- afresh without them; a table whose comment named one is not made afresh for its comment alone. Each statement below
-- that defines something is written exactly as db/schema.sql writes it.

DROP VIEW item_verifications;
DROP VIEW work_owner_words;

DROP VIEW item_acceptance;
CREATE VIEW item_acceptance AS
  -- The item's acceptance, and the round that gave it, by its work and its number in the work; NULL until the work
  -- goes on to implement: until then what its creator wanted is `wanted` of item_body. locked says only whether there
  -- is one. Read by: cells `I1`, `T2`, `T3`, `T1` and `T6`.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;

DROP VIEW work_open_questions;
CREATE VIEW work_open_questions AS
  -- Each work's open questions as its design handed them in, each a matter of the project's architect, with the answer
  -- that came, which is the architect's reply: its own judgment, or the owner's answer it carried. Read by: cells `D1`
  -- and `D2`; the engine, at cell `M1`.
  SELECT m.work, m.id AS matter, m.what AS question, json_extract(m.body, '$.answer') AS answer
  FROM architect_matters m
  WHERE m.kind = 'open_question';
