-- Takes a version 65 database to version 66: the run guard keeps every call a running round makes, what each round came
-- to, and each signal that went past its line. A round from before the ledger has no calls, so what its record tells (its
-- first and last prompts, its turns, the message it was given) is put in `round_measures`, marked as not from the ledger,
-- so that a graph's lines have a history from the start. Each statement below that defines something is written exactly
-- as db/schema.sql writes it.

CREATE TABLE round_calls (
  -- The ledger of the agents' calls to the model: one row for each call a running round makes, as its command line's
  -- stream tells of it, however many lines of the stream give the call. The row is written when the stream first tells
  -- of the call and its output tokens follow as the stream tells them, so that it holds what a round used even when the
  -- round is stopped before the command line's report at its end, and how the context moved from call to call, which
  -- that report does not tell. A round from before the ledger has none.
  -- Hidden from agents.
  -- Read by: the run guard, for the session's last prompt when a round starts; the owner's interface, for the rounds
  -- running now.
  -- The round: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The call's place among the round's, from 1, in the order the stream first told of each.
  seq INTEGER NOT NULL CHECK (seq >= 1),
  -- The id of the message the call wrote, which tells a call apart however many lines give it.
  message TEXT NOT NULL,
  -- The model it ran on, as the stream names it.
  model TEXT,
  -- Its input tokens: not read from the cache, read from it, and written to it for five minutes and for an hour.
  input INTEGER NOT NULL CHECK (input >= 0),
  read INTEGER NOT NULL CHECK (read >= 0),
  written_5m INTEGER NOT NULL CHECK (written_5m >= 0),
  written_1h INTEGER NOT NULL CHECK (written_1h >= 0),
  -- Its output tokens: the most the stream told.
  output INTEGER NOT NULL CHECK (output >= 0),
  -- 1 for a subagent's call, which is not in the context of the round's session.
  subagent INTEGER NOT NULL CHECK (subagent IN (0, 1)),
  -- When the stream first told of it.
  at INTEGER NOT NULL,
  PRIMARY KEY (work, round, seq),
  UNIQUE (work, round, message),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE round_measures (
  -- What each agent's round that made a call came to, by what the run guard's lines are computed from: one row for each
  -- round, written once when it ended, or put in from its record by the migration that brought the ledger. A round the
  -- guard stopped has none, so that an anomaly never becomes its own baseline.
  -- Hidden from agents.
  -- Read by: the run guard, which computes a round's lines from these rows when it starts.
  -- The round: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The work's graph and the round's cell, as the flow version it ran on names them.
  graph TEXT NOT NULL,
  cell TEXT NOT NULL,
  -- The model it ran on, as the command line reported it.
  model TEXT,
  -- Its calls to the model, those of subagents left out. Not from the ledger: the turns the command line reported, which
  -- may be more than its calls.
  calls INTEGER NOT NULL CHECK (calls >= 1),
  -- The tokens its context grew by: every rise between neighbouring calls added up, so that a compaction takes nothing
  -- back. Not from the ledger: its last prompt less its first, which a compaction hides.
  added INTEGER NOT NULL CHECK (added >= 0),
  -- The tokens its first prompt was past the last prompt of its session before it; NULL for a session's first round.
  jump INTEGER CHECK (jump IS NULL OR jump >= 0),
  -- The characters the engine gave it: its message and the return before it in its session; NULL when its record does
  -- not tell.
  given INTEGER CHECK (given IS NULL OR given >= 0),
  -- The tool uses that repeated an earlier one of the round exactly; NULL when not from the ledger, since nothing kept
  -- tool uses before it.
  repeats INTEGER CHECK (repeats IS NULL OR repeats >= 0),
  -- How many times the command line compacted the session during it.
  compactions INTEGER NOT NULL CHECK (compactions >= 0),
  -- What it cost, in US dollars; NULL when its record does not tell.
  cost_usd REAL,
  -- 1 when kept call by call as it ran, 0 when put in from its record. A cell's own line reads only the former.
  ledger INTEGER NOT NULL CHECK (ledger IN (0, 1)),
  PRIMARY KEY (work, round),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE guard_events (
  -- Each signal of a running round that went past its line, once in a round: what the run guard saw and did, kept as
  -- every anomaly is. Written once.
  -- Read by: the owner's interface, which lists them under anomalies; agents and the architect tracing what the guard
  -- did to a round.
  id INTEGER PRIMARY KEY,
  -- The round: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The signal: growth, the context added in the round, with how much a call; jump, the first prompt past the session's
  -- last, with how much for each character the engine gave; cost; calls; repeats, tool uses repeated exactly;
  -- compactions.
  signal TEXT NOT NULL CHECK (signal IN ('growth', 'jump', 'cost', 'calls', 'repeats', 'compactions')),
  -- Its value and its line: tokens, US dollars or a count, as the signal is.
  value REAL NOT NULL,
  line REAL NOT NULL,
  -- For growth and jump, the second condition: its value and line. NULL for the signals of one condition.
  also REAL,
  also_line REAL,
  -- How many rounds gave the line, and whether they were the cell's own or the whole graph's.
  samples INTEGER NOT NULL CHECK (samples >= 1),
  scope TEXT NOT NULL CHECK (scope IN ('cell', 'graph')),
  -- What came of it: noted, recorded and the round went on; stopped, the round was stopped; waived, it would have been
  -- stopped, but the owner let the work go on after the cell's last round was stopped.
  action TEXT NOT NULL CHECK (action IN ('noted', 'stopped', 'waived')),
  -- When it went past.
  at INTEGER NOT NULL,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

-- The rounds a cell's and a graph's lines are computed from.
CREATE INDEX round_measures_by_cell ON round_measures (graph, cell);

INSERT INTO round_measures
  (work, round, graph, cell, model, calls, added, jump, given, repeats, compactions, cost_usd, ledger)
WITH told AS MATERIALIZED (
  SELECT r.work, r.n, w.graph, r.cell, r.model, s.cli_session, r.started_at, r.compactions,
    length(r.output) AS returned,
    coalesce(json_extract(r.usage, '$.turns'), 1) AS turns,
    json_extract(r.usage, '$.cost_usd') AS cost,
    coalesce(json_extract(r.usage, '$.first_call.input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.first_call.cache_read_input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.first_call.cache_creation_input_tokens'), 0) AS opening,
    coalesce(json_extract(r.usage, '$.usage.iterations[#-1].input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.usage.iterations[#-1].cache_read_input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.usage.iterations[#-1].cache_creation_input_tokens'), 0) AS closing,
    CASE json_type(r.input, '$.message')
      WHEN 'text' THEN length(json_extract(r.input, '$.message'))
      WHEN 'object' THEN (
        SELECT sum(CASE WHEN e.type = 'text' THEN length(e.value)
          ELSE coalesce((SELECT length(c.text) FROM contents c WHERE c.hash = json_extract(e.value, '$.content')), 0) END)
        FROM json_each(r.input, '$.message.parts') e)
    END AS message
  FROM rounds r
  JOIN works w ON w.id = r.work
  JOIN sessions s ON s.id = r.session
  WHERE r.cell IS NOT NULL AND r.usage IS NOT NULL
),
ran AS (
  SELECT *, lag(closing) OVER q AS closed_at, lag(returned) OVER q AS returned_before
  FROM told
  WHERE opening > 0 AND closing > 0
  WINDOW q AS (PARTITION BY cli_session ORDER BY started_at, work, n)
)
SELECT work, n, graph, cell, model, max(turns, 1), max(closing - opening, 0),
  CASE WHEN closed_at IS NOT NULL THEN max(opening - closed_at, 0) END,
  message + coalesce(returned_before, 0), NULL, compactions, cost, 0
FROM ran;
