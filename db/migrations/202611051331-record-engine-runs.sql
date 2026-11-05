-- Takes a database to one that records each run of the engine's loop, from its start to its stop, in the new table
-- engine_runs. A run before the migration left no row. The statement below that defines something is written exactly as
-- db/schema.sql writes it.

CREATE TABLE engine_runs (
  -- Each time the engine's loop (`wf-agents run`) ran on this database, from its start to its stop, so that a gap in
  -- which no work moved can be told to be a stop and a start of the engine or not: when it started, which build and
  -- configuration it ran, and why it stopped. A row is written as the loop starts and completed as it stops. A start
  -- that finds a row never completed marks it lost: the engine's process ended without recording its stop (it was
  -- killed, or the machine went down), and when is not known. Two engines are not run on one database at once.
  -- Read by: the owner and the architect, through the tool `query`, looking at why works stood still; the engine
  -- reads it only to mark the rows a start finds never completed.
  id INTEGER PRIMARY KEY,
  -- When the loop started.
  started_at INTEGER NOT NULL,
  -- The commit the engine was built from, the one every round records as the engine's version.
  built_from TEXT NOT NULL,
  -- The commit of the configuration the loop started with. A committed change applies from the next round on, and each
  -- round says which commit it ran with.
  config_commit TEXT NOT NULL,
  -- The version the flow built into this build is stored as.
  flow_version INTEGER NOT NULL REFERENCES flow_versions(id),
  -- When the loop stopped; NULL while it runs, and for a row marked lost.
  stopped_at INTEGER,
  -- Why it stopped; NULL while it runs. `stop file`: the owner, or whoever installs a new build, asked it to stop, and
  -- it dispatched nothing more and waited for the jobs in flight to end. `error: ` and the text: the loop ended on an
  -- error it could not deal with. `lost`: the row was found never completed, as above.
  stop_reason TEXT,
  CHECK ((stop_reason IS NULL) = (stopped_at IS NULL) OR stop_reason = 'lost')
) STRICT;
