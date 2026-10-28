-- Takes a version 22 database to version 23. What an agent told the owner was kept twice, as the agent's message and
-- as a notice of the engine's that repeated it; the engine now keeps the agent's message alone, and each notice that
-- repeats a message of its work word for word is removed. Nothing is defined here.

-- The change is the migration's, not a writer's, so the trigger that records deletions from the notices is dropped
-- first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_engine_notices_delete;
DELETE FROM engine_notices WHERE ask IS NULL AND fault IS NULL AND EXISTS (
  SELECT 1 FROM owner_messages m
  WHERE m.work = engine_notices.work AND length(engine_notices.text) > length(m.text)
    AND substr(engine_notices.text, length(engine_notices.text) - length(m.text) + 1) = m.text
    AND substr(engine_notices.text, 1, length(engine_notices.text) - length(m.text)) LIKE 'The % at cell % tells you: ');
