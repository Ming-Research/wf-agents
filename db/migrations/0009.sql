-- Takes a version 8 database to version 9. An earlier engine, updating a work's design card, left the card before it
-- open or ruled beside the new one; each such card is replaced by the next card of its work, as the engine now does.
-- Nothing is defined here.

-- The change is the migration's, not a writer's, so the trigger that records changes to design cards is dropped
-- first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_design_cards_update;
UPDATE design_cards SET status = 'replaced', replaced_by = (
  SELECT min(n.id) FROM design_cards n JOIN rounds nr ON nr.id = n.round
  WHERE nr.work = (SELECT r.work FROM rounds r WHERE r.id = design_cards.round) AND n.id > design_cards.id)
WHERE status <> 'replaced' AND EXISTS (
  SELECT 1 FROM design_cards n JOIN rounds nr ON nr.id = n.round
  WHERE nr.work = (SELECT r.work FROM rounds r WHERE r.id = design_cards.round) AND n.id > design_cards.id);
