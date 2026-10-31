-- Takes a version 14 database to version 15: the session manager. A session says why it started, and each keep-warm
-- ping the session manager sends a waiting session is kept, with where the work waited and what the ping cost. Each
-- statement below that defines something is written exactly as db/schema.sql writes it.

ALTER TABLE sessions ADD COLUMN started TEXT NOT NULL DEFAULT 'line' CHECK (started IN ('line', 'cell', 'limit', 'wake'));

-- A session started before is the line's own, but a wake's row that goes on in the CLI session an earlier row has,
-- which is a wake going on in the architect's session. The change is the migration's, not a writer's, so the trigger
-- that records changes to sessions is dropped first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_sessions_update;
UPDATE sessions SET started = 'wake' WHERE line = 'architect' AND EXISTS (
  SELECT 1 FROM sessions e WHERE e.cli_session = sessions.cli_session AND e.id < sessions.id);

CREATE TABLE pings (
  -- Each keep-warm ping: one minimal message the session manager sent a session whose work waited, 55 minutes after
  -- the session's last call, so that the session's prompt cache was read before it expired instead of written again
  -- whole at its next round. A ping means the work waited over an hour, which is an anomaly: it says where the work
  -- waited and for what. Its cost is the work's.
  -- Hidden from agents.
  -- Read by: the session manager, which counts a wait's pings and a session's cost; the owner's interface and the
  -- anomalies command.
  id INTEGER PRIMARY KEY,
  session INTEGER NOT NULL REFERENCES sessions(id),
  -- Where the work waited and for what, JSON: the cell, what it waited on (pause, owner, architect, items or tool)
  -- and that in words.
  waited TEXT NOT NULL CHECK (json_valid(waited)),
  -- The model the ping ran on, as the CLI reported it.
  model TEXT,
  -- Tokens and cost as the CLI reported them, JSON, with the ping's own cost as a round's usage has it; NULL while it
  -- runs.
  usage TEXT CHECK (usage IS NULL OR json_valid(usage)),
  -- How many times the CLI compacted the session during the ping: the session then holds nothing from before.
  compactions INTEGER NOT NULL DEFAULT 0 CHECK (compactions >= 0),
  -- Why the run failed, when it did.
  error TEXT,
  -- When it started and ended.
  started_at INTEGER NOT NULL,
  ended_at INTEGER
) STRICT;

-- The keep-warm pings of a session.
CREATE INDEX pings_by_session ON pings (session);
