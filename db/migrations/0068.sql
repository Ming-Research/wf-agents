-- Takes a version 67 database to version 68: each round keeps the counter the row it took counts (rounds.counts), and
-- the views of counters read it from there, not from the compiled flow, which they parsed again for every round. The
-- rounds already written get it by the rule the views had: the row named by the round's cell and transition, in the
-- graph of the work, in the flow version the work is on now. A flow version whose rows could not be told apart by that
-- rule makes the migration fail. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

ALTER TABLE rounds ADD COLUMN counts TEXT CHECK (counts IS NULL OR transition IS NOT NULL);

-- The rows that count something, in the flow versions works are on, read from each compiled flow once. Two rows of one
-- cell with the same name that both count something fail the primary key: the old views counted such a round twice,
-- which one column cannot hold.
CREATE TEMP TABLE counted_rows (
  version INTEGER NOT NULL,
  graph TEXT NOT NULL,
  cell TEXT NOT NULL,
  name TEXT NOT NULL,
  counts TEXT NOT NULL,
  PRIMARY KEY (version, graph, cell, name)
) WITHOUT ROWID;
INSERT INTO temp.counted_rows
  SELECT v.id, json_extract(g.value, '$.name'), json_extract(c.value, '$.id'), json_extract(x.value, '$.name'),
    json_extract(x.value, '$.counts')
  FROM flow_versions v
  JOIN json_each(v.compiled, '$.graphs') g
  JOIN json_each(g.value, '$.cells') c
  JOIN json_each(c.value, '$.rows') x
  WHERE v.id IN (SELECT flow_version FROM works)
    AND json_extract(g.value, '$.name') IS NOT NULL AND json_extract(c.value, '$.id') IS NOT NULL
    AND json_extract(x.value, '$.name') IS NOT NULL AND json_extract(x.value, '$.counts') IS NOT NULL;

UPDATE rounds SET counts = k.counts
  FROM works w JOIN temp.counted_rows k ON k.version = w.flow_version AND k.graph = w.graph
  WHERE w.id = rounds.work AND k.cell = rounds.cell AND k.name = rounds.transition;

DROP TABLE temp.counted_rows;

CREATE INDEX rounds_counts ON rounds (work, counts) WHERE counts IS NOT NULL;

DROP VIEW work_asking;
DROP VIEW work_counters;

CREATE VIEW work_counters AS
  -- How many times each work took the rows that count each counter, by the counter each round recorded when it ended
  -- (`rounds.counts`); and its total time, in minutes, which no row counts: the time of its agents' rounds and of the
  -- tools they had the engine run (cell `X1`). Read by: cell `A1`; the engine, against the project's limits; the
  -- owner's interface.
  SELECT r.work, r.counts AS counter, count(*) AS times
  FROM rounds r
  WHERE r.counts IS NOT NULL AND r.counts <> 'total_time'
  GROUP BY r.work, r.counts
  UNION ALL
  SELECT r.work, 'total_time' AS counter, sum(r.ended_at - r.started_at) / 60000 AS times
  FROM rounds r
  WHERE r.ended_at IS NOT NULL AND (r.session IS NOT NULL OR r.cell IN ('X1', '13', 'tools', 'TOOLS'))
  GROUP BY r.work;

CREATE VIEW work_asking AS
  -- The cell that asked, or that took up an interjection, and how many times each work has asked: what choosing the
  -- way on needs of where the work stands. Read by: cell `A4`.
  SELECT w.id AS work, w.resume_at AS asked_at,
    (SELECT count(*) FROM rounds r WHERE r.work = w.id AND r.counts = 'asks') AS asks
  FROM works w;
