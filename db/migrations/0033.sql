-- Takes a version 32 database to version 33: a word to a work that needs no answer, as the owner confirming every
-- decision of a design card, is marked so; and each work's total time is measured from its rounds, in minutes, not
-- counted by the rows that pause. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

ALTER TABLE interjections ADD COLUMN informs INTEGER NOT NULL DEFAULT 0 CHECK (informs IN (0, 1));

DROP VIEW work_asking;
DROP VIEW work_counters;

CREATE VIEW work_counters AS
  -- How many times each work took the rows that count each counter, by the compiled flow it runs on; and its total
  -- time, in minutes, which no row counts: the time of its agents' rounds and of the tools they had the engine run (cell
  -- `X1`). Read by: cell `A1`; cell `A4`, through work_asking; the engine, against the project's limits; the owner's
  -- interface.
  SELECT r.work, json_extract(row.value, '$.counts') AS counter, count(*) AS times
  FROM rounds r
  JOIN works w ON w.id = r.work
  JOIN flow_versions v ON v.id = w.flow_version
  JOIN json_each(v.compiled, '$.graphs') g ON json_extract(g.value, '$.name') = w.graph
  JOIN json_each(g.value, '$.cells') c ON json_extract(c.value, '$.id') = r.cell
  JOIN json_each(c.value, '$.rows') row ON json_extract(row.value, '$.name') = r.transition
  WHERE json_extract(row.value, '$.counts') IS NOT NULL AND json_extract(row.value, '$.counts') <> 'total_time'
  GROUP BY r.work, counter
  UNION ALL
  SELECT r.work, 'total_time' AS counter, sum(r.ended_at - r.started_at) / 60000 AS times
  FROM rounds r
  WHERE r.ended_at IS NOT NULL AND (r.session IS NOT NULL OR r.cell IN ('X1', '13', 'tools', 'TOOLS'))
  GROUP BY r.work;

CREATE VIEW work_asking AS
  -- The cell that asked, or that took up an interjection, and how many times each work has asked: what choosing the
  -- way on needs of where the work stands. Read by: cell `A4`.
  SELECT w.id AS work, w.resume_at AS asked_at,
    coalesce((SELECT c.times FROM work_counters c WHERE c.work = w.id AND c.counter = 'asks'), 0) AS asks
  FROM works w;
