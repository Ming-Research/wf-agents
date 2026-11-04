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

-- architect_matters is made afresh, as its description says the matters the engine takes in a fixed order and its
-- two new columns hold the decision log's line and the effects held for the second eye.
CREATE TABLE architect_matters_before_forks AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, main's CI red, a place its plan of what to do next found to set straight, or the
  -- check of its policy. The project's queue is its matters not handled; the engine takes them in a fixed order, by
  -- what waits on each: the owner's words and replies first, then main's CI red, then what a work stopped waits on, the
  -- one that came first; then the base when it is due; then what the plan queued; then what waited for the planning.
  -- Each is handled in a session of its own forked from the base. A matter something waits on starts a work of the
  -- project's architect when none runs; one that waits for the planning starts one only once it came before the base
  -- was built, or once as many wait as the installation's amount.
  -- Read by: the engine, which starts the architect's works, takes each matter, routes it to its path, hands it its
  -- material and takes its replies back; the architect's cells through architect_queue and work_handled; the owner's
  -- interface.
  id INTEGER PRIMARY KEY,
  -- The project whose architect it is for.
  project TEXT NOT NULL,
  -- Its kind, which decides the path it takes: set_straight, engineer_question, open_question, report, design_card,
  -- failure, owner_words, other_architect, slots_free, main_red (main's CI red, which the engine's watch of main
  -- queues: its body holds the commit, its red checks and their runs, and while one is open the project's merge queue
  -- is stopped), or check_policy, the check of the policy in force, one open at a time; slots_free and make_plan (the
  -- plan of what to do next, which a flow version before queued) are what a planning answers. An open question is what an engineer's design could not settle: its body holds the question, the options, the
  -- one recommended, the link to the design (design_link) and, once it has come, the answer (answer) with who gave it
  -- (answered_by: architect, or owner when the architect pinned the question for the owner and the owner answered), for
  -- the owner the ask that holds their words (owner_ask), and the time it came (answered_at, in milliseconds). A
  -- matter handled before these kinds keeps the kind it had then: set_policy for the setting of a new policy, which a
  -- flow version before ran as a stage of its own, and policy for a setting that ended, when the setting was a work of
  -- its own.
  kind TEXT NOT NULL,
  -- What more its kind says: an engineer's question's own kind (split, duplicate, ...); a report's (a side report's
  -- kind, followup for a report's what to do next, answer for an engineer's answer to what the architect said to its
  -- work, flow_feedback for what agents said was wrong with their cells, or fault for a fault of wf-agents forwarded
  -- to it, by the owner or by the owner's setting, which starts a work of it as a report does); a failure's
  -- (tool_failed, environment_changed); the owner's words' (said, answer, release, approved_changes); a message's
  -- (message, reply, or flow_feedback for flow feedback another project's architect forwarded); the kind of a place to
  -- set straight. NULL when there is nothing more. Read by: the engine, for the matter's material and whether it starts a work of
  -- the architect; the owner's interface.
  detail TEXT,
  -- What it is, in one sentence, as the engine wrote it.
  what TEXT NOT NULL,
  -- Its material, JSON: what the asking cell handed in, the side report, a report's what to do next with its paths and
  -- the commit merged, the design card, the tool that failed, the owner's words (with where they were said, and who
  -- the caller declared to speak: `speaker`, owner or delegate; none for words kept before it was declared), the
  -- message, the place to set straight; for a fault forwarded, who forwarded it (by: owner, or setting for the owner's
  -- setting), the fault itself being read from its notice when the matter is handled. Read by: the engine, for the
  -- matter's material.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The item it concerns; NULL when it concerns none.
  item INTEGER REFERENCES items(id),
  -- The round that raised it: an engineer's round at the architect's cell, a round that handed in a side report or a
  -- design card, an architect's round; NULL for one the owner or the engine raised.
  work INTEGER,
  round INTEGER,
  -- For a fault of wf-agents forwarded to it: the engine's notice that reports the fault, which goes to each
  -- project's architect once; NULL for any other matter. Read by: the engine, which hands the architect the notice
  -- and the engine's evidence of the fault as they stand when the matter is handled, and adds a further cache miss to
  -- a notice forwarded while its matter waits; the owner's interface, which shows where each fault went and how it
  -- stands there.
  notice INTEGER REFERENCES engine_notices(id),
  -- When it came. Read by: the queue, for how long it waited.
  at INTEGER NOT NULL,
  -- The ask of the owner a cell on its path put about it, which holds it while the owner has not answered: it is then
  -- out of the queue, and once answered it is back, and goes back to the cell that asked, with the answer. NULL when no
  -- cell asked about it. Read by: the queue; the engine, which routes it and gives the answer.
  asked INTEGER REFERENCES owner_asks(id),
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the approval of
  -- the draft of a new policy, the cell that wrote it, a reply of exactly approved going to where it takes effect
  -- instead.
  resume_cell TEXT,
  -- For a matter of two cells that its first cell sent back to the queue, as a flow version before did so that the
  -- owner's words that came meanwhile were taken first: its second cell, where it goes on (`H3` after `H2`, `H5` after
  -- `H4`, `P2` after `P1`). No cell sets it now, since a matter's cells follow one another in its own session; it is
  -- cleared once the matter is closed or a cell on its path asks the owner about it. Read by: the engine, which routes
  -- a matter that still holds one there.
  goes_on_at TEXT,
  -- For a side report or an engineer's answer a flow version before had the architect pick together with others of
  -- its kind in one pick, the matter that led the group: the one picked. No pick groups matters now, each being handled
  -- in a session of its own; it stays on a matter handled so, saying with which others it was handled. Read by: the
  -- owner's interface.
  group_lead INTEGER REFERENCES architect_matters(id),
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. The owner's answer in the
  -- work's group to a question the architect holds is the reply too, said by the owner (by), with who the caller
  -- declared to speak (speaker, owner or delegate). Read by: the engine, which takes it back unchanged to the work that
  -- asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the second eye, JSON, as the round handed it in: it becomes the reply, with the numbers of
  -- the items made, once the second eye finds the round's changes sound. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its work and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; the owner ruled the design card it was to look at, or another card replaced it, saying which;
  -- the owner answered the question in the work's group, the reply saying what; it said that a setting of the
  -- policy, a work of its own then, had ended, when the setting became a stage of the architect's works; or it was a
  -- setting of the policy as a stage of its own, which runs so no longer. NULL otherwise.
  -- Read by: architect_queue, which leaves it out; the engine, which tells a new work on the item what an earlier
  -- work asked and had withdrawn; the owner's interface, which leaves it out of the queue.
  withdrawn TEXT,
  -- What the session that handled it left as it ended, JSON: its line of the decision log (`log`), and its answers
  -- whether the matter changes the plan (`plan`: no or yes) and whether something that matters in the base no longer
  -- holds (`base`: holds or wrong), each with why (`plan_why`, `base_why`), with the round that handed them in
  -- (`round`). NULL until then, and for a matter handled before the architect's matters were forked. Read by: the
  -- engine, which gives every matter handled after it the decision log since the base, and counts the answers among
  -- the facts that call for a new base; the owner's interface.
  logged TEXT CHECK (logged IS NULL OR json_valid(logged)),
  -- What the round handed in that waits for the second eye, JSON: its effects and its answer, made once the second eye
  -- finds the changes sound, or dropped when it sends them back. NULL otherwise. Read by: the engine.
  effects_held TEXT CHECK (effects_held IS NULL OR json_valid(effects_held)),
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO architect_matters (id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, goes_on_at, group_lead, reply, reply_held, handled_work, handled_round, withdrawn)
  SELECT id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, goes_on_at, group_lead, reply, reply_held, handled_work, handled_round, withdrawn FROM architect_matters_before_forks;
DROP TABLE architect_matters_before_forks;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;

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
