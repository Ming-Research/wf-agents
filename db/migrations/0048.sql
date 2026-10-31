-- Takes a version 47 database to version 48: a line starts a new session when the agents' login account changed, the
-- CLI's configuration directory its rounds run with, since the session is kept in the directory it ran in and its calls
-- are made under the login there; so a session's start may say account, and the sessions are made afresh with their
-- rows. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE sessions_version_47 AS SELECT * FROM sessions;
DROP TABLE sessions;

CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session's context is past the configuration's limit, when the model its rounds run on
  -- changed, whose prompt cache it would have to write again whole, when the agents' login account changed, whose
  -- configuration directory of the CLI does not hold the session, and when the owner asks for one. Each project's
  -- architect's line goes on in one CLI session across the works of the project's architect, the setting of a new
  -- policy included: each has a row of its own with the same cli_session. A cell on no line has a session of its own
  -- for each round, ended once the round has.
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
  -- session before; carried, a work of a project's architect going on in the CLI session the project's previous
  -- architect work left; account, the line's rounds run under another login account of the CLI than its session
  -- before, in another configuration directory, which does not hold that session;
  -- owner, the owner asked for a new session with the round's Start or Step on the page. Read by: the owner's
  -- interface.
  started TEXT NOT NULL DEFAULT 'line' CHECK (started IN ('line', 'cell', 'limit', 'model', 'carried', 'account', 'owner'))
) STRICT;

INSERT INTO sessions (id, work, line, cli, cli_session, ended, started)
SELECT id, work, line, cli, cli_session, ended, started
FROM sessions_version_47;
DROP TABLE sessions_version_47;
CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;
