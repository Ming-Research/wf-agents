-- Takes a version 55 database to version 56: the architect's look at a design card leaves its queue, withdrawn saying
-- why, as the owner rules the card, disagreeing with a decision as much as confirming them all, or as another card
-- replaces it, instead of the look at the queue closing it when the owner confirmed the card or it was replaced. A look
-- still in the queue at a card already ruled or replaced is withdrawn here the same way. Nothing it defines changes.

-- The changes below are the migration's, not a writer's, so the triggers that record changes to the rows they
-- write are dropped first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_architect_matters_update;

UPDATE architect_matters SET withdrawn = (
  SELECT CASE WHEN c.status = 'ruled' THEN 'the owner ruled design card ' || c.id
              WHEN c.replaced_by IS NOT NULL THEN 'design card ' || c.id || ' was replaced by design card '
                || c.replaced_by
              ELSE 'design card ' || c.id || ' was replaced' END
  FROM design_cards c WHERE c.id = json_extract(architect_matters.body, '$.design_card'))
WHERE kind = 'design_card' AND handled_round IS NULL AND withdrawn IS NULL
  AND json_extract(body, '$.design_card') IN (SELECT id FROM design_cards WHERE status IN ('ruled', 'replaced'));
