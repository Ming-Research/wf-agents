-- Takes a version 65 database to version 66: the rows the engine takes for a round that ended in no return its cell
-- takes (a failed run, a round cut off, a return sent back) are counted by the counters the flow's rounds section
-- names, so the view of the row each round took is new, and the counters read from it. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.

DROP VIEW work_asking;
DROP VIEW work_counters;

CREATE VIEW round_rows AS
  -- The row each round of a work took at its cell: the row its return took; for a round that ended in no return its
  -- cell takes, the shared row the engine took for it, which the round does not record as a transition, since a
  -- transition says the cell's own return was taken: the kind of the failed run (`<kind>_too_often` for the one past
  -- the kind's bound), `cut_off` for a round the engine cut off, `sent_back` for a return sent back. None for a round
  -- still open and for one that ended with the work paused. Read by: the engine, which counts how often a work took a
  -- row in a row; work_counters.
  SELECT work, n AS round, cell,
    CASE
      WHEN transition IS NOT NULL THEN transition
      WHEN json_extract(rejected, '$.failed.over') IS NOT NULL THEN json_extract(rejected, '$.failed.kind') || '_too_often'
      WHEN json_extract(rejected, '$.failed.kind') IS NOT NULL THEN json_extract(rejected, '$.failed.kind')
      WHEN json_extract(rejected, '$.interrupted') IS NOT NULL THEN 'cut_off'
      WHEN json_extract(rejected, '$.error') IS NOT NULL THEN NULL
      WHEN rejected IS NOT NULL THEN 'sent_back'
    END AS took
  FROM rounds;

CREATE VIEW work_counters AS
  -- How many times each work took the rows that count each counter, by the compiled flow it runs on; and its total
  -- time, in minutes, which no row counts: the time of its agents' rounds and of the tools they had the engine run (cell
  -- `X1`). Read by: cell `A1`; cell `A4`, through work_asking; the engine, against the project's limits; the owner's
  -- interface.
  SELECT r.work, json_extract(row.value, '$.counts') AS counter, count(*) AS times
  FROM round_rows r
  JOIN works w ON w.id = r.work
  JOIN flow_versions v ON v.id = w.flow_version
  JOIN json_each(v.compiled, '$.graphs') g ON json_extract(g.value, '$.name') = w.graph
  JOIN json_each(g.value, '$.cells') c ON json_extract(c.value, '$.id') = r.cell
  JOIN json_each(c.value, '$.rows') row ON json_extract(row.value, '$.name') = r.took
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
