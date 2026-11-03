-- Takes a version 73 database to version 74: a round hands its session's progress over to a new session once keeping
-- the session has cost the round as much as starting anew, which a session records as why it started (handover), and a
-- round the engine may ask to hand over records, once it came due, where and what it came to, which the rounds'
-- definition says.
-- The sessions table says the new reason in a check, which cannot be changed in place, so it is made afresh with its
-- rows, and so are the rounds, whose definition says what their record holds. A migration runs with the foreign keys
-- off, as SQLite makes a table afresh, and every one is checked before the migration commits. Each statement below
-- that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE sessions_version_73 AS SELECT * FROM sessions;
DROP TABLE sessions;
CREATE TABLE sessions (
  -- The CLI session each line of a work runs in. A line goes on in its session and lets the CLI compact it, after
  -- which the session manager gives the cell's rules and inputs again; it starts a new session at a cell the flow says
  -- it starts one at, when its session hands its progress over to a new one, keeping it having cost as much as starting
  -- anew, when its session's context is past the configuration's limit, when the model its rounds run on
  -- changed, whose prompt cache it would have to write again whole, when the agents' login account changed, whose
  -- configuration directory of the CLI does not hold the session, when the CLI's prompt cache let it go (a line the
  -- flow says expires with the cache, its session last called a cache lifetime or more ago), and when the owner asks
  -- for one. Each project's architect's line goes on in one CLI session across the works of the project's architect,
  -- the setting of a new policy included: each has a row of its own with the same cli_session. A cell on no line has a
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
  -- it; owner, the owner asked for a new session with the round's Start or Step on the page; handover, a round of the
  -- line's session came due, its own calls having paid as much as starting anew would cost, and handed its progress
  -- over: the hand-over cell opened this one for the cell that handed over. Read by: the owner's interface.
  started TEXT NOT NULL DEFAULT 'line'
    CHECK (started IN ('line', 'cell', 'limit', 'model', 'carried', 'account', 'expired', 'owner', 'handover'))
) STRICT;
INSERT INTO sessions (id, work, line, cli, cli_session, ended, started)
  SELECT id, work, line, cli, cli_session, ended, started FROM sessions_version_73;
DROP TABLE sessions_version_73;
CREATE UNIQUE INDEX sessions_going_on ON sessions (work, line) WHERE ended IS NULL;

CREATE TABLE rounds_version_73 AS SELECT * FROM rounds;
DROP TABLE rounds;
CREATE TABLE rounds (
  -- One round: one CLI run of an agent, from the message the engine sends to the return the agent hands in, or to a
  -- run that ends without one; or one step the engine takes itself, one run of a tool cell, one answer from the owner.
  -- A pause, a question and a request for tools each end a round. The row is inserted when the round starts,
  -- completed when it ends, and never changed after. A round is known by its work and its number in the work, and
  -- named R<work>-<n>: R5-9 is the ninth round of work 5.
  -- Read by: every cell input that names a field another cell returned; the engine, to tell what each session already
  -- holds; the views of counters, anomalies and time; the owner's history command.
  -- The work, and the round's number in it: the work's rounds are counted from 1 in the order they started.
  work INTEGER NOT NULL REFERENCES works(id),
  n INTEGER NOT NULL CHECK (n >= 1),
  -- The cell; NULL for a round off the graph, in which a line that is not running only answers an interjection.
  cell TEXT,
  -- The session the agent ran in; NULL for the engine's own steps, tool cells and the owner.
  session INTEGER REFERENCES sessions(id),
  -- What the engine gave, JSON. For an agent's round: the message, word for word; each conditional input in it, by its
  -- address and the hash of its content, and whether it was given or only referred to; and the call (the whole
  -- command line, the session it started or resumed, the engine's version and the command line's). For another step,
  -- the values of its inputs; for cell `start`, nothing. Rounds recorded before this format keep the prompt and inputs.
  -- A round the engine may ask to hand its session's progress over records, once it came due, `$.engine.handover`:
  -- due_at, the call it came due at by its seq in round_calls, 1 or more; rent and buy, what the round's own calls had
  -- paid to read again what the session holds past its first prompt and what writing its context to the cache again
  -- would cost then, in US dollars; tools_after, the tool uses after it came due, after each of which the hook asked;
  -- opening, the session's first prompt; last, as the round ended, its last call's prompt and model and the rent and
  -- the buy then. Its transition says whether it handed over. A round never due has none.
  -- Read by: the engine, to tell what a session already holds; the owner's interface.
  input TEXT NOT NULL CHECK (json_valid(input)),
  -- The whole return, JSON: for the engine's own step what it computed, for a tool cell the tools' results, for the
  -- owner the answer. NULL while the round runs. Read by: every input that names a returned field.
  output TEXT CHECK (output IS NULL OR json_valid(output)),
  -- The transition row the return took. NULL while the round runs, and when the return was rejected.
  -- Read by: the engine; the view of anomalies.
  transition TEXT,
  -- Why the engine sent the return back or the run failed, JSON: a check it failed or a wrong format; or a run that
  -- ended without a return, with its kind as the flow names them. NULL when the return was accepted.
  -- Read by: the engine, which counts returns sent back apart from failed runs; the anomalies view.
  rejected TEXT CHECK (rejected IS NULL OR json_valid(rejected)),
  -- The commit of the configuration the round ran with. The engine reads only committed configuration, so this
  -- names every setting behind the round, including those that shaped the engine's own decisions and never reached
  -- an agent's input. Read by: people and agents tracing why a round went the way it did.
  config_commit TEXT NOT NULL,
  -- The model the round ran on; NULL when no model ran. Recorded each round because the configuration can change.
  model TEXT,
  -- Tokens and cost as the CLI reported them, JSON. Read by: the views of cost.
  usage TEXT CHECK (usage IS NULL OR json_valid(usage)),
  -- How many times the CLI compacted the session during the round. Read by: the anomalies view.
  compactions INTEGER NOT NULL DEFAULT 0 CHECK (compactions >= 0),
  -- When the round started and ended. Read by: the views of time.
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  -- The counter the row this round took counts, as the flow version the work ran on said it when the round ended; the
  -- row is the one the return took (`transition`) or, for a round that ended in no return its cell takes, the shared
  -- row the engine took for it (view `round_rows`). NULL when the round took no row, or the row counts nothing. A
  -- round's row can be found again in the flow only by the version it ran on, which a round does not record and a
  -- work leaves when it moves to a newer one, so the counter is kept with the round, written in the same step as
  -- `transition` or `rejected`. 'total_time' is kept as the row says it, though the views count no round for it.
  -- Read by: the views of counters and of how many times a work asked, through the index `rounds_counts`.
  counts TEXT CHECK (counts IS NULL OR transition IS NOT NULL OR rejected IS NOT NULL),
  PRIMARY KEY (work, n),
  CHECK (transition IS NULL OR rejected IS NULL)
) STRICT;

-- The rows keep the order they had, which the rounds' rowids follow.
INSERT INTO rounds (work, n, cell, session, input, output, transition, rejected, config_commit, model, usage,
    compactions, started_at, ended_at, counts)
  SELECT work, n, cell, session, input, output, transition, rejected, config_commit, model, usage, compactions,
    started_at, ended_at, counts
  FROM rounds_version_73;
DROP TABLE rounds_version_73;

CREATE INDEX rounds_counts ON rounds (work, counts) WHERE counts IS NOT NULL;
