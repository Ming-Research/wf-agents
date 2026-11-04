-- Takes a database to one whose keep-warm pings say what a work waited on as the engine judges it where the work
-- stood: a free slot (slot) once what it waited for came, the holder of its open blocker, or what the row it took
-- waits for (row), where a ping before named the items its item depends on at any cell where a row waits. The pings
-- are made afresh with their rows, as the column's description is part of the table's definition, which a migrated
-- database holds as a new one does; a migration runs with the foreign keys off, and every one is checked before the
-- migration commits. The pings already recorded keep what they said. Each statement below that defines something is
-- written exactly as db/schema.sql writes it.

CREATE TABLE pings_before_slot AS SELECT * FROM pings;
DROP TABLE pings;

CREATE TABLE pings (
  -- Each keep-warm ping: one minimal message the session manager sent an idle session of a line its work could come
  -- back to, 55 minutes after the session's last call, so that the session's prompt cache was read before it expired
  -- instead of written again whole at the line's next round. A ping means the line waited over an hour, while its work
  -- waited or went on without it: it says where the work stood and for what. Its cost is the work's.
  -- Hidden from agents.
  -- Read by: the session manager, which counts a wait's pings and a session's cost; the owner's interface and the
  -- anomalies command.
  id INTEGER PRIMARY KEY,
  session INTEGER NOT NULL REFERENCES sessions(id),
  -- Where the work stood and for what the line waited, JSON: the cell, what the work waited on (pause, slot, owner,
  -- architect, items or row) or work when it went on without the line, and that in words.
  waited TEXT NOT NULL CHECK (json_valid(waited)),
  -- The model the ping ran on, as the CLI reported it.
  model TEXT,
  -- Tokens and cost as the CLI reported them, JSON, with the ping's own cost as a round's usage has it; NULL while it
  -- runs.
  usage TEXT CHECK (usage IS NULL OR json_valid(usage)),
  -- How many times the CLI compacted the session during the ping: the session then holds nothing from before but its
  -- system prompt.
  compactions INTEGER NOT NULL DEFAULT 0 CHECK (compactions >= 0),
  -- Why the run failed, when it did.
  error TEXT,
  -- When it started and ended.
  started_at INTEGER NOT NULL,
  ended_at INTEGER
) STRICT;

INSERT INTO pings (id, session, waited, model, usage, compactions, error, started_at, ended_at)
  SELECT id, session, waited, model, usage, compactions, error, started_at, ended_at FROM pings_before_slot;
DROP TABLE pings_before_slot;

CREATE INDEX pings_by_session ON pings (session);
