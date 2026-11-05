-- Takes a database to one where the health numbers can read the waits for a free slot by time: the indexes on when a
-- wait began and when it ended, like those of the blockers. The table slot_waits, whose description now says that the
-- health numbers read it, is made afresh with its rows, as a migration runs with the foreign keys off. Each statement
-- below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE slot_waits_before_stops AS SELECT * FROM slot_waits;
DROP TABLE slot_waits;

CREATE TABLE slot_waits (
  -- Each stretch of time a work waited for a free slot: what it waited for had come (the owner's or the architect's
  -- answer, or what the row it took waited for), and it waited on, holding no slot, until its machine had one free.
  -- The engine's loop opens a row as it reaps a step of the work that found its wait over and was granted no slot, and
  -- closes it as it grants the work a slot or finds the work ended, paused or gone on.
  -- Read by: the engine's loop, which grants the works with an open row a free slot before any new work starts; the
  -- owner's interface, through the engine's status, which names such a work as waiting for a free slot; the engine's
  -- health numbers, which count such a wait among the times a work stood stopped, as `slot`; whoever looks at why
  -- works stood still.
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

INSERT INTO slot_waits (id, work, since, until, ended)
  SELECT id, work, since, until, ended FROM slot_waits_before_stops;
DROP TABLE slot_waits_before_stops;

-- The open wait for a free slot of each work: one at most.
CREATE UNIQUE INDEX slot_waits_open ON slot_waits (work) WHERE until IS NULL;
-- The waits for a free slot begun within a time, and those that ended after a time, which the health numbers read a
-- period by.
CREATE INDEX slot_waits_by_time ON slot_waits (since);
CREATE INDEX slot_waits_by_end ON slot_waits (until);
