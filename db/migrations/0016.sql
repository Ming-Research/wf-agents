-- Takes a version 15 database to version 16: a line's session also starts anew when the model its rounds run on
-- changed, which a session records as why it started. The table is made afresh with its rows, since a check cannot be
-- changed in place. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE sessions_version_15 AS SELECT * FROM sessions;
DROP TABLE sessions;
CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session's context is past the configuration's limit, and when the model its rounds run
  -- on changed, whose prompt cache it would have to write again whole. The architect's line goes on
  -- in one CLI session across its wakes: each wake has a row of its own with the same cli_session, until a new policy
  -- ends it. A cell on no line has a session of its own for each round, ended once the round has.
  -- Hidden from agents.
  -- Read by: the session manager, to resume a line, to start a new session and to keep a waiting session warm; the
  -- owner's interface.
  id INTEGER PRIMARY KEY,
  work INTEGER NOT NULL REFERENCES works(id),
  -- The line, for example engineer, review, adversarial, advisor or architect; for a cell on no line, the cell.
  line TEXT NOT NULL,
  -- The CLI that started the session. Only that CLI can resume it, so this stays when the configuration changes.
  cli TEXT NOT NULL,
  -- The CLI's own id for the session, passed to it to resume.
  cli_session TEXT NOT NULL,
  -- When the session was ended, so that the line's next round starts a new one; NULL while it goes on. A work's line
  -- has at most one session going on. Read by: the engine, which resumes only a session not ended.
  ended INTEGER,
  -- Why it started: line, the line had no session going on (its first round in the work, or after its session
  -- ended); cell, the round is the work's first at a cell the line starts a new session at; limit, the context of the
  -- line's session before it was past the configuration's limit; model, the line's rounds run on another model than its
  -- session before; wake, a wake of the architect going on in the CLI session the last wake left. Read by: the owner's
  -- interface.
  started TEXT NOT NULL DEFAULT 'line' CHECK (started IN ('line', 'cell', 'limit', 'model', 'wake'))
) STRICT;
INSERT INTO sessions (id, work, line, cli, cli_session, ended, started)
  SELECT id, work, line, cli, cli_session, ended, started FROM sessions_version_15;
DROP TABLE sessions_version_15;
-- The session each line of a work goes on in: one at most, among those not ended.
CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;
