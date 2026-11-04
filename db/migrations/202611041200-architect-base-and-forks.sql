-- Takes a database to one whose architect handles each matter in a session forked from its base, the session that
-- read the project last. A session may be a fork (`started` fork) of another (`fork_of`), and a session of a line whose
-- sessions are each a matter's names its matter (`matter`): the sessions are made afresh with their rows, as SQLite
-- makes a table afresh to change what a column may hold, with the foreign keys off, every one of them checked before
-- the migration commits. Each matter keeps the line of the decision log its session left as it ended (`logged`) and
-- what waits for the second eye (`effects_held`). Flow feedback no longer goes to the architect, and what was said in
-- the works' groups is given to each matter's session as what changed since the base: such matters not handled yet
-- leave the queue, saying why. The view of the queue says what reads it now, and the view of how often main was red
-- only on known red checks counts the architect's watch of main too. Each statement below that defines something is
-- written exactly as db/schema.sql writes it.

CREATE TABLE sessions_before_forks AS SELECT * FROM sessions;
DROP TABLE sessions;

CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session's context is past the configuration's limit, when the model its rounds run on
  -- changed, whose prompt cache it would have to write again whole, when the agents' login account changed, whose
  -- configuration directory of the CLI does not hold the session, when the CLI's prompt cache let it go (a line the
  -- flow says expires with the cache, its session last called a cache lifetime or more ago), and when the owner asks
  -- for one. Each project's architect's base, the session that read the project last, goes on in one CLI session across
  -- the works of the project's architect: each has a row of its own with the same cli_session. A matter of the
  -- architect is handled in a session of its own, forked from the base: the CLI copies the base's session under a new
  -- id, so that it holds all the base read, and the fork's rounds are read after the base's. A cell on no line has a
  -- session of its own for each round, ended once the round has.
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
  -- before, in another configuration directory, which does not hold that session; expired, the CLI's prompt cache
  -- let the line's session go, its last call a cache lifetime or more before, on a line the flow says expires with
  -- it; owner, the owner asked for a new session with the round's Start or Step on the page; fork, a matter's first
  -- round on a line that forks from another, forked from that line's latest session, the base. Read by: the owner's
  -- interface.
  started TEXT NOT NULL DEFAULT 'line'
    CHECK (started IN ('line', 'cell', 'limit', 'model', 'carried', 'account', 'expired', 'owner', 'fork')),
  -- The session it was forked from, the base, for a session started as fork: it holds what the base held when it was
  -- forked, and its rounds are read after the base's. NULL for any other. Read by: the session manager, to tell what a
  -- fork holds and what it cost.
  fork_of INTEGER REFERENCES sessions(id),
  -- The matter of a project's architect the session is for, on a line whose sessions are each a matter's: a matter
  -- parked by a question to the owner takes its session up again when the answer comes, in whichever work of the
  -- architect runs then. NULL on any other line. Read by: the session manager, to find a matter's session.
  matter INTEGER REFERENCES architect_matters(id),
  CHECK ((started = 'fork') = (fork_of IS NOT NULL))
) STRICT;

INSERT INTO sessions (id, work, line, cli, cli_session, ended, started)
  SELECT id, work, line, cli, cli_session, ended, started FROM sessions_before_forks;
DROP TABLE sessions_before_forks;

CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;

ALTER TABLE architect_matters ADD COLUMN logged TEXT CHECK (logged IS NULL OR json_valid(logged));
ALTER TABLE architect_matters ADD COLUMN effects_held TEXT CHECK (effects_held IS NULL OR json_valid(effects_held));

UPDATE architect_matters
  SET withdrawn = 'Flow feedback no longer goes to the architect: it stays in the returns of the rounds it is about, for the owner and the delegate to take up.'
  WHERE handled_round IS NULL AND withdrawn IS NULL AND kind IN ('report', 'other_architect') AND detail = 'flow_feedback';
UPDATE architect_matters
  SET withdrawn = 'What was said in the works'' groups is no longer a matter of its own: each matter''s session is given it as what changed since the base.'
  WHERE handled_round IS NULL AND withdrawn IS NULL AND kind = 'group_talk';
UPDATE architect_matters SET group_lead = NULL WHERE handled_round IS NULL AND group_lead IS NOT NULL;

DROP VIEW architect_queue;

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell for its question; the owner, for
  -- the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer and to which cell, the second cell a flow version before had a matter of two cells go on at, and whether
  -- its reporter marked it as harming now (urgent).
  -- Read by: cells `S0` and `Q0`; the engine, which takes `S0`'s rows by it for each project whose architect runs no
  -- work, and takes the next matter by it in its fixed order; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AND m.goes_on_at IS NULL AS answered,
    m.resume_cell AS back_to,
    m.goes_on_at,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;

DROP VIEW known_red_counts;

CREATE VIEW known_red_counts AS
  -- How often main was red only on known red checks, by project: as the architect's watch of main checked it, and as
  -- each engineer's work watched main after its merge before main was watched for the project. Read by: the owner's
  -- interface.
  SELECT coalesce(i.project, w.project) AS project, count(*) AS times
  FROM rounds r JOIN works w ON w.id = r.work LEFT JOIN items i ON i.id = w.item
  WHERE r.cell IN ('L1', '10', 'watch_main') AND json_extract(r.output, '$.main.kind') = 'known_red'
  GROUP BY coalesce(i.project, w.project);
