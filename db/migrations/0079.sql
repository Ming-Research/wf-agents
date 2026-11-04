-- Takes a version 78 database to version 79: a work's wait for a free slot, until now kept in the engine's memory alone,
-- is recorded, each stretch of it a row of the new table slot_waits. A wait under way as the database is migrated was
-- lost with the engine that stopped, and the work's next step finds it again. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

CREATE TABLE slot_waits (
  -- Each stretch of time a work waited for a free slot: what it waited for had come (the owner's or the architect's
  -- answer, or what the row it took waited for), and it waited on, holding no slot, until its machine had one free.
  -- The engine's loop opens a row as it reaps a step of the work that found its wait over and was granted no slot, and
  -- closes it as it grants the work a slot or finds the work ended, paused or gone on.
  -- Read by: the engine's loop, which grants the works with an open row a free slot before any new work starts; the
  -- owner's interface, through the engine's status, which names such a work as waiting for a free slot; whoever looks
  -- at why works stood still.
  id INTEGER PRIMARY KEY,
  -- The work that waited.
  work INTEGER NOT NULL REFERENCES works(id),
  -- When its wait for a slot began: when the step that found its wait over ended.
  since INTEGER NOT NULL,
  -- When its wait for a slot ended; NULL while it lasts.
  until INTEGER,
  -- How it ended: granted, the loop granted the work a free slot; ended, the work ended; paused, the work was paused;
  -- gone_on, the work went on without a slot, as by a round the owner asked for, or no longer waited where it had, as
  -- when it was brought to a newer flow version. NULL while it lasts.
  ended TEXT CHECK (ended IN ('granted', 'ended', 'paused', 'gone_on')),
  CHECK ((until IS NULL) = (ended IS NULL))
) STRICT;

CREATE UNIQUE INDEX slot_waits_open ON slot_waits (work) WHERE until IS NULL;
