-- Takes a version 78 database to version 79: the item changes of an architect's batch have a number of their own, a
-- line, which every revised version of a change keeps and which the review and the architect's revision name a change
-- by, where they named it by its place in the list; the engine tries the batch out before each look of the review and
-- keeps what it found in the new table architect_batch_views. A change that edits an item names the parts it changes,
-- each by its own field, instead of one part and its text, and the change that edited only the acceptance is one of
-- them; a hold carries the text that is written into the item beside the reason the review reads. The changes already
-- kept, those still waiting for their review and the history of the batches made, are written in the new shapes, so
-- that nothing reads the old ones after this. Each statement below that defines something is written exactly as
-- db/schema.sql writes it.

-- The rewrite below is the migration's, not a writer's, so the trigger that records changes to the rows it writes is
-- dropped first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_architect_item_changes_update;

ALTER TABLE architect_item_changes ADD COLUMN line INTEGER;

-- A batch that was waiting for its review is numbered by the places its changes had, which the review's rows name.
UPDATE architect_item_changes SET line = (
  SELECT count(*) FROM architect_item_changes x
  WHERE x.work = architect_item_changes.work AND x.state = 'collected' AND x.id <= architect_item_changes.id)
WHERE state = 'collected';

-- A change of one part and its text names the part as the field that holds the text.
UPDATE architect_item_changes SET change = json_remove(
  json_set(change, '$.' || json_extract(change, '$.part'), json_extract(change, '$.text')), '$.part', '$.text')
WHERE json_extract(change, '$.kind') = 'edit_item' AND json_extract(change, '$.part') IS NOT NULL;

-- A change of the acceptance alone is a change that edits the item and names the acceptance.
UPDATE architect_item_changes SET change = json_set(change, '$.kind', 'edit_item')
WHERE json_extract(change, '$.kind') = 'edit_acceptance';

-- A hold wrote its reason into the item: that is its text.
UPDATE architect_item_changes SET change = json_set(change, '$.text', json_extract(change, '$.reason'))
WHERE json_extract(change, '$.kind') = 'hold' AND json_extract(change, '$.text') IS NULL;

DROP INDEX architect_item_changes_collected;
CREATE UNIQUE INDEX architect_item_changes_by_line ON architect_item_changes (work, line) WHERE state = 'collected';
CREATE INDEX architect_item_changes_by_round ON architect_item_changes (work, round);

CREATE TABLE architect_batch_views (
  -- What a project's architect's batch of item changes comes to, as the engine tried it out before each look of the
  -- review: one row for each work, written over at every look and deleted when the batch is over. The engine tries the
  -- changes in order in a savepoint it rolls back, so that nothing of them stays, and reads what each wrote from
  -- `changes`. The review is given this as its inputs `batch_changes` and `batch_after`, and the next look is compared
  -- with the row left by the one before. Not tracked in `changes`: it is the engine's bookkeeping, and gone with its
  -- batch.
  -- Read by: the engine, which gives it to the review at cell `B2` and compares the next look with it.
  work INTEGER PRIMARY KEY REFERENCES works(id),
  -- The round whose move to the review wrote it.
  round INTEGER NOT NULL,
  -- What each change was at this look, JSON object by line: its change and its effect, which the next look is compared
  -- with.
  lines TEXT NOT NULL CHECK (json_valid(lines)),
  -- Where each change stands against the look before, JSON object by line: new, changed, effect_changed or unchanged,
  -- and, for the changes the look before had that this one does not, removed with why. Given to the review beside each
  -- change.
  states TEXT NOT NULL CHECK (json_valid(states)),
  -- What the changes make of the items, JSON: what each change did, the items they name or make as they stand after
  -- them, where those stand in their project's order, and the items that are ready then, in the order they start.
  after TEXT NOT NULL CHECK (json_valid(after)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;
