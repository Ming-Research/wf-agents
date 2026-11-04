-- The queries behind research/ci-speed-measured.md that read the engine's database (table tool_runs). They are run with
-- the engine's `query` tool, one statement at a time (the database is not in the repository), and each is the text that
-- was run; the report names them Q1 to Q10. Whom it serves: a reader who wants to re-run a number of the report; it is
-- removed with the report.
--
-- Times in tool_runs are UTC milliseconds. The moments used:
--   1793691504000  2026-11-03 07:38:24 UTC, when "Build the workspace's own crates unoptimized in CI" (3a9ea8d9) was
--                  committed to main (a work took it up only when it next merged main)
--   1793703148000  2026-11-03 10:52:28 UTC, the switch: the first `ci` call with named tests (`push.run` is set)
--   1793827133000  2026-11-04 21:18:53 UTC, the cutoff that freezes the sets (the last `ci` of the sample ran at 21:18:52);
--                  every query but Q1b and Q10 (see there) filters `queued_at < 1793827133000`, so that a re-run gives
--                  the same sets
-- A `ci` or `ci_before` call is a row of tool_runs; "10 s or more" leaves out the refused calls and those that only
-- hand in the pull request's text.

-- Q1. `ci` calls by period: n, mean, median, min, max, how many ran in 180 s or less.
WITH b AS (
  SELECT CASE WHEN json_extract(result,'$.push.run') IS NULL
              THEN CASE WHEN queued_at < 1793691504000 THEN 'A whole CI, before the unoptimized profile'
                        ELSE 'B whole CI, after it, before the switch' END
              ELSE 'C named tests (fast CI)' END AS g,
         (ended_at-started_at)/1000.0 AS x, queued_at AS qa
  FROM tool_runs
  WHERE tool='ci' AND ended_at IS NOT NULL AND ended_at-started_at >= 10000
    AND json_extract(result,'$.push.verdict')='pushed' AND queued_at < 1793827133000),
r AS (SELECT g, x, qa, row_number() OVER (PARTITION BY g ORDER BY x) AS rn, count(*) OVER (PARTITION BY g) AS n FROM b)
SELECT g, n, round(avg(x),1) AS mean_s,
       round(avg(CASE WHEN rn IN ((n+1)/2,(n+2)/2) THEN x END),1) AS median_s,
       round(min(x),1) AS min_s, round(max(x),1) AS max_s, sum(x<=180) AS n_le_180,
       datetime(min(qa)/1000,'unixepoch') AS first_at, datetime(max(qa)/1000,'unixepoch') AS last_at
FROM r GROUP BY g ORDER BY g;

-- Q1b. The same calls by what they asked for (`push.run`: named tests with the lint, with a named check, alone, the
-- lint only; empty for the whole CI), run at 2026-11-04 21:03 UTC, so without the calls after that. It has no cutoff
-- of its own, so a re-run counts the calls after 21:03 too.
WITH c AS (
  SELECT CASE WHEN json_extract(result,'$.push.run') IS NULL
              THEN CASE WHEN queued_at < 1793691480000 THEN 'whole_before_opt0' ELSE 'whole_after_opt0' END
              ELSE json_extract(result,'$.push.run') END AS regime,
         (ended_at-started_at)/1000.0 AS s
  FROM tool_runs WHERE tool='ci' AND ended_at IS NOT NULL AND (ended_at-started_at) >= 10000),
r AS (SELECT regime, s, row_number() OVER (PARTITION BY regime ORDER BY s) AS rn,
             count(*) OVER (PARTITION BY regime) AS n FROM c)
SELECT regime, n, round(avg(s)) AS mean_s, round(avg(CASE WHEN rn IN ((n+1)/2, (n+2)/2) THEN s END),1) AS median_s,
       round(min(s)) AS min_s, round(max(s)) AS max_s
FROM r GROUP BY regime ORDER BY regime;

-- Q2. The two values that bound the median's 95% interval: the values at ranks j and k of Q1's sets, with
-- j = floor((n - 1.96 sqrt n) / 2) and k = ceil(1 + (n + 1.96 sqrt n) / 2): n 440 -> 199, 242; 30 -> 9, 22;
-- 470 (A and B together) -> 213, 258; 161 -> 68, 94.
WITH b AS (
  SELECT CASE WHEN json_extract(result,'$.push.run') IS NULL THEN CASE WHEN queued_at < 1793691504000 THEN 'A' ELSE 'B' END
              ELSE 'C' END AS g, (ended_at-started_at)/1000.0 AS x
  FROM tool_runs
  WHERE tool='ci' AND ended_at IS NOT NULL AND ended_at-started_at >= 10000
    AND json_extract(result,'$.push.verdict')='pushed' AND queued_at < 1793827133000),
b2 AS (SELECT g, x FROM b UNION ALL SELECT 'AB', x FROM b WHERE g IN ('A','B')),
r AS (SELECT g, x, row_number() OVER (PARTITION BY g ORDER BY x) AS rn, count(*) OVER (PARTITION BY g) AS n FROM b2)
SELECT g, n, rn, x FROM r
WHERE (g='A' AND rn IN (199,242)) OR (g='B' AND rn IN (9,22)) OR (g='AB' AND rn IN (213,258)) OR (g='C' AND rn IN (68,94))
ORDER BY g, rn;

-- Q3. Rounds that asked for both `ci` and `ci_before` (the first row of each tool in the round): how many, the mean
-- minutes of each, the mean of the shorter of the two (the most the overlap can save), and the mean span of the round.
-- "sequential": ci_before started after ci ended; "overlapped": it started before.
WITH p AS (
  SELECT c.work, c.round, c.queued_at AS q, c.started_at AS cs, c.ended_at AS ce, b.started_at AS bs, b.ended_at AS be
  FROM tool_runs c
  JOIN tool_runs b ON b.work=c.work AND b.round=c.round AND b.tool='ci_before' AND b.attempt=1 AND b.ended_at IS NOT NULL
    AND b.id=(SELECT min(x.id) FROM tool_runs x WHERE x.work=b.work AND x.round=b.round AND x.tool='ci_before')
  WHERE c.tool='ci' AND c.attempt=1 AND c.ended_at IS NOT NULL AND c.queued_at < 1793827133000
    AND c.id=(SELECT min(x.id) FROM tool_runs x WHERE x.work=c.work AND x.round=c.round AND x.tool='ci')),
vv AS (
  SELECT 'sequential, all' AS variant, * FROM p WHERE bs >= ce
  UNION ALL SELECT 'sequential, ci >= 10 s', * FROM p WHERE bs >= ce AND ce-cs >= 10000
  UNION ALL SELECT 'sequential, ci queued before 2026-11-03 00:00 UTC', * FROM p WHERE bs >= ce AND q < 1793664000000
  UNION ALL SELECT 'sequential, ci queued before 2026-11-03 09:10 UTC', * FROM p WHERE bs >= ce AND q < 1793697000000
  UNION ALL SELECT 'overlapped, all', * FROM p WHERE bs < ce
  UNION ALL SELECT 'overlapped, ci >= 10 s', * FROM p WHERE bs < ce AND ce-cs >= 10000)
SELECT variant, count(*) AS rounds, round(avg((ce-cs)/60000.0),2) AS ci_min, round(avg((be-bs)/60000.0),2) AS before_min,
       round(avg(min(ce-cs, be-bs)/60000.0),2) AS shorter_min, round(avg((max(ce,be)-min(cs,bs))/60000.0),2) AS span_min
FROM vv GROUP BY variant ORDER BY variant;

-- Q3b. When the first overlapped round and the last sequential one were queued (the same p as in Q3).
WITH p AS (
  SELECT c.work, c.round, c.queued_at AS q, c.started_at AS cs, c.ended_at AS ce, b.started_at AS bs, b.ended_at AS be
  FROM tool_runs c
  JOIN tool_runs b ON b.work=c.work AND b.round=c.round AND b.tool='ci_before' AND b.attempt=1 AND b.ended_at IS NOT NULL
    AND b.id=(SELECT min(x.id) FROM tool_runs x WHERE x.work=b.work AND x.round=b.round AND x.tool='ci_before')
  WHERE c.tool='ci' AND c.attempt=1 AND c.ended_at IS NOT NULL AND c.queued_at < 1793827133000
    AND c.id=(SELECT min(x.id) FROM tool_runs x WHERE x.work=c.work AND x.round=c.round AND x.tool='ci'))
SELECT bs < ce AS overlapped, count(*) AS rounds, datetime(min(q)/1000,'unixepoch') AS first_ci_queued,
       datetime(max(q)/1000,'unixepoch') AS last_ci_queued
FROM p GROUP BY 1;

-- Q4. After the switch, 10 s or more: `ci` and `ci_before` calls by whether the other tool was asked for in the same
-- round: n, mean, median, min, max.
WITH base AS (
  SELECT t.tool, (t.ended_at-t.started_at)/1000.0 AS x,
         CASE WHEN t.tool='ci'
              THEN EXISTS(SELECT 1 FROM tool_runs o WHERE o.work=t.work AND o.round=t.round AND o.tool='ci_before')
              ELSE EXISTS(SELECT 1 FROM tool_runs o WHERE o.work=t.work AND o.round=t.round AND o.tool='ci') END AS together
  FROM tool_runs t
  WHERE t.tool IN ('ci','ci_before') AND t.ended_at IS NOT NULL AND t.ended_at-t.started_at >= 10000
    AND t.started_at >= 1793703148000 AND t.queued_at < 1793827133000
    AND (json_extract(t.result,'$.push.verdict')='pushed' OR json_extract(t.result,'$.ran.verdict')='ran')),
r AS (SELECT tool || CASE together WHEN 1 THEN ' with the other in the round' ELSE ' alone in the round' END AS g, x,
             row_number() OVER (PARTITION BY tool, together ORDER BY x) AS rn,
             count(*) OVER (PARTITION BY tool, together) AS n FROM base)
SELECT g, n, round(avg(x),1) AS mean_s, round(avg(CASE WHEN rn IN ((n+1)/2,(n+2)/2) THEN x END),1) AS median_s,
       round(min(x),1) AS min_s, round(max(x),1) AS max_s
FROM r GROUP BY g ORDER BY g;

-- Q4b. The values at the ranks of the 95% interval of Q4's medians, with the cutoff that freezes the sets
-- (n 58 -> 21, 38; 103 -> 41, 63; 11 -> 2, 10; 68 -> 25, 44).
WITH base AS (
  SELECT t.tool, (t.ended_at-t.started_at)/1000.0 AS x,
         CASE WHEN t.tool='ci'
              THEN EXISTS(SELECT 1 FROM tool_runs o WHERE o.work=t.work AND o.round=t.round AND o.tool='ci_before')
              ELSE EXISTS(SELECT 1 FROM tool_runs o WHERE o.work=t.work AND o.round=t.round AND o.tool='ci') END AS together
  FROM tool_runs t
  WHERE t.tool IN ('ci','ci_before') AND t.ended_at IS NOT NULL AND t.ended_at-t.started_at >= 10000
    AND t.started_at >= 1793703148000 AND t.queued_at < 1793827133000
    AND (json_extract(t.result,'$.push.verdict')='pushed' OR json_extract(t.result,'$.ran.verdict')='ran')),
r AS (SELECT tool, together, x, row_number() OVER (PARTITION BY tool, together ORDER BY x) AS rn,
             count(*) OVER (PARTITION BY tool, together) AS n FROM base)
SELECT tool, together, n, rn, x FROM r
WHERE (tool='ci' AND together=1 AND rn IN (21,38)) OR (tool='ci' AND together=0 AND rn IN (41,63))
   OR (tool='ci_before' AND together=0 AND rn IN (2,10)) OR (tool='ci_before' AND together=1 AND rn IN (25,44))
ORDER BY tool, together, rn;

-- Q5. Each merged work: when its first `ci` and its merge were, how many `ci` calls it made (all, and after the
-- switch), the minutes it waited in the four tools (sum of their durations; union of their intervals, so that the time
-- ci and ci_before overlapped counts once), and the minutes in each tool. "Merged" is a merge row whose result is a
-- 40-character commit id.
WITH m AS (
  SELECT work, max(ended_at) AS merged_at FROM tool_runs
  WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND queued_at < 1793827133000 GROUP BY work),
t AS (
  SELECT work, tool, started_at AS s, ended_at AS e,
         max(ended_at) OVER (PARTITION BY work ORDER BY started_at, id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING) AS prev
  FROM tool_runs WHERE tool IN ('ci','ci_before','full_ci','merge') AND ended_at IS NOT NULL
    AND queued_at < 1793827133000),
w AS (
  SELECT work, min(CASE WHEN tool='ci' THEN s END) AS first_ci, sum(tool='ci') AS n_ci,
         sum(CASE WHEN tool='ci' AND s >= 1793703148000 THEN 1 ELSE 0 END) AS n_ci_post,
         sum(e-s)/60000.0 AS sum_min, sum(max(0, e-max(s, coalesce(prev, s))))/60000.0 AS union_min,
         sum(CASE WHEN tool='ci' THEN e-s ELSE 0 END)/60000.0 AS ci_min,
         sum(CASE WHEN tool='ci_before' THEN e-s ELSE 0 END)/60000.0 AS b_min,
         sum(CASE WHEN tool='full_ci' THEN e-s ELSE 0 END)/60000.0 AS f_min,
         sum(CASE WHEN tool='merge' THEN e-s ELSE 0 END)/60000.0 AS m_min
  FROM t GROUP BY work)
SELECT w.work, datetime(w.first_ci/1000,'unixepoch') AS first_ci, datetime(m.merged_at/1000,'unixepoch') AS merged_at,
       w.n_ci, w.n_ci_post, round(w.sum_min,1) AS sum_min, round(w.union_min,1) AS union_min, round(w.ci_min,1) AS ci_min,
       round(w.b_min,1) AS b_min, round(w.f_min,1) AS f_min, round(w.m_min,1) AS m_min
FROM w JOIN m USING(work) WHERE w.first_ci IS NOT NULL ORDER BY m.merged_at;

-- Q6. The two cohorts of Q5 compared: PRE25 is the 25 works merged last before the switch, POST the works whose first
-- `ci` was after it and that are merged. Number, mean, median, min, max of the minutes (sum, union) and of the `ci`
-- calls of a work, and how many works waited 34.8 minutes or less.
WITH m AS (
  SELECT work, max(ended_at) AS merged_at FROM tool_runs
  WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND queued_at < 1793827133000 GROUP BY work),
t AS (
  SELECT work, tool, started_at AS s, ended_at AS e,
         max(ended_at) OVER (PARTITION BY work ORDER BY started_at, id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING) AS prev
  FROM tool_runs WHERE tool IN ('ci','ci_before','full_ci','merge') AND ended_at IS NOT NULL
    AND queued_at < 1793827133000),
w AS (
  SELECT work, min(CASE WHEN tool='ci' THEN s END) AS first_ci, sum(tool='ci') AS n_ci, sum(e-s)/60000.0 AS sum_min,
         sum(max(0, e-max(s, coalesce(prev, s))))/60000.0 AS union_min,
         sum(CASE WHEN tool='ci' THEN e-s ELSE 0 END)/60000.0 AS ci_min,
         sum(CASE WHEN tool='ci_before' THEN e-s ELSE 0 END)/60000.0 AS b_min,
         sum(CASE WHEN tool='full_ci' THEN e-s ELSE 0 END)/60000.0 AS f_min,
         sum(CASE WHEN tool='merge' THEN e-s ELSE 0 END)/60000.0 AS m_min
  FROM t GROUP BY work),
c AS (
  SELECT 'PRE25: the 25 works merged last before the switch' AS g, work, n_ci, sum_min, union_min, ci_min, b_min, f_min, m_min
  FROM (SELECT w.*, m.merged_at, row_number() OVER (ORDER BY m.merged_at DESC) AS k
        FROM w JOIN m USING(work) WHERE m.merged_at < 1793703148000 AND w.first_ci IS NOT NULL) WHERE k <= 25
  UNION ALL
  SELECT 'POST: first ci after the switch, merged', w.work, w.n_ci, w.sum_min, w.union_min, w.ci_min, w.b_min, w.f_min, w.m_min
  FROM w JOIN m USING(work) WHERE w.first_ci >= 1793703148000),
vals AS (
  SELECT g, 'sum_min' AS metric, sum_min AS x FROM c UNION ALL SELECT g, 'union_min', union_min FROM c
  UNION ALL SELECT g, 'n_ci', n_ci FROM c),
r AS (SELECT g, metric, x, row_number() OVER (PARTITION BY g, metric ORDER BY x) AS rn,
             count(*) OVER (PARTITION BY g, metric) AS n FROM vals)
SELECT g, metric, n, round(avg(x),1) AS mean, round(avg(CASE WHEN rn IN ((n+1)/2,(n+2)/2) THEN x END),1) AS median,
       round(min(x),1) AS min, round(max(x),1) AS max, sum(x<=34.8) AS le_34_8
FROM r GROUP BY g, metric ORDER BY g, metric;

-- Q6b. The same two cohorts: the calls and the minutes in each tool, per work and per `ci` call.
WITH m AS (
  SELECT work, max(ended_at) AS merged_at FROM tool_runs
  WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND queued_at < 1793827133000 GROUP BY work),
t AS (SELECT work, tool, started_at AS s, ended_at AS e FROM tool_runs
      WHERE tool IN ('ci','ci_before','full_ci','merge') AND ended_at IS NOT NULL AND queued_at < 1793827133000),
w AS (
  SELECT work, min(CASE WHEN tool='ci' THEN s END) AS first_ci, sum(tool='ci') AS n_ci, sum(e-s)/60000.0 AS sum_min,
         sum(CASE WHEN tool='ci' THEN e-s ELSE 0 END)/60000.0 AS ci_min,
         sum(CASE WHEN tool='ci_before' THEN e-s ELSE 0 END)/60000.0 AS b_min,
         sum(CASE WHEN tool='full_ci' THEN e-s ELSE 0 END)/60000.0 AS f_min,
         sum(CASE WHEN tool='merge' THEN e-s ELSE 0 END)/60000.0 AS m_min
  FROM t GROUP BY work),
c AS (
  SELECT 'PRE25: the 25 works merged last before the switch' AS g, work, n_ci, sum_min, ci_min, b_min, f_min, m_min
  FROM (SELECT w.*, m.merged_at, row_number() OVER (ORDER BY m.merged_at DESC) AS k
        FROM w JOIN m USING(work) WHERE m.merged_at < 1793703148000 AND w.first_ci IS NOT NULL) WHERE k <= 25
  UNION ALL
  SELECT 'POST: first ci after the switch, merged', w.work, w.n_ci, w.sum_min, w.ci_min, w.b_min, w.f_min, w.m_min
  FROM w JOIN m USING(work) WHERE w.first_ci >= 1793703148000)
SELECT g, count(*) AS works, sum(n_ci) AS ci_calls, round(sum(ci_min),1) AS ci_min,
       round(sum(ci_min)/sum(n_ci),2) AS ci_min_per_call, round(sum(b_min),1) AS before_min,
       round(sum(f_min),1) AS full_min, round(sum(m_min),1) AS merge_min, round(sum(sum_min),1) AS total_min,
       round(sum(sum_min)/count(*),1) AS per_work_min
FROM c GROUP BY g;

-- Q6c. The values of PRE25's sum_min at ranks 7 and 19, the bounds of the median's 95% interval for n 25. With the
-- CTEs m and t of Q6 and w reduced to work, first_ci and sum_min:
WITH m AS (
  SELECT work, max(ended_at) AS merged_at FROM tool_runs
  WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND queued_at < 1793827133000 GROUP BY work),
t AS (SELECT work, tool, started_at AS s, ended_at AS e FROM tool_runs
      WHERE tool IN ('ci','ci_before','full_ci','merge') AND ended_at IS NOT NULL AND queued_at < 1793827133000),
w AS (SELECT work, min(CASE WHEN tool='ci' THEN s END) AS first_ci, sum(e-s)/60000.0 AS sum_min FROM t GROUP BY work),
pre AS (SELECT work, sum_min, row_number() OVER (ORDER BY merged_at DESC) AS k
        FROM (SELECT w.work, w.sum_min, m.merged_at FROM w JOIN m USING(work)
              WHERE m.merged_at < 1793703148000 AND w.first_ci IS NOT NULL)),
r AS (SELECT work, sum_min, row_number() OVER (ORDER BY sum_min) AS rn FROM pre WHERE k <= 25)
SELECT rn, work, round(sum_min,1) AS sum_min FROM r WHERE rn IN (7,19) ORDER BY rn;

-- Q7. The 25 works of the baseline pull request (works 238 to 268 that ran `ci`), by the moment their tool times are
-- counted up to: how many works and `ci` calls, and the minutes in each tool.
WITH cut(label, c) AS (
  VALUES ('2026-11-02 12:00 UTC',1793620800000),('2026-11-02 18:00 UTC',1793642400000),('2026-11-03 00:00 UTC',1793664000000),
         ('2026-11-03 06:00 UTC',1793685600000),('2026-11-03 09:00 UTC',1793696400000),('to the cutoff',1793827133000))
SELECT cut.label, count(DISTINCT t.work) AS works, sum(t.tool='ci') AS ci_calls,
       round(sum(CASE WHEN t.tool='ci' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS ci_min,
       round(sum(CASE WHEN t.tool='ci_before' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS before_min,
       round(sum(CASE WHEN t.tool='full_ci' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS full_min,
       round(sum(CASE WHEN t.tool='merge' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS merge_min,
       round(sum(t.ended_at-t.started_at)/60000.0,1) AS total_min
FROM cut JOIN tool_runs t
  ON t.work BETWEEN 238 AND 268 AND t.queued_at < cut.c AND t.ended_at IS NOT NULL
 AND t.tool IN ('ci','ci_before','full_ci','merge') AND t.work IN (SELECT work FROM tool_runs WHERE tool='ci')
GROUP BY cut.label ORDER BY cut.c;

-- Q7b. The same 25 works counted up to each of their own `ci` calls in turn (ordered by queued_at), for calls 247 to
-- 250: where the baseline pull request's 249 calls fall. A call and a tool row count when it was queued at or before
-- that call, and has ended by now.
WITH ci AS (
  SELECT queued_at AS q, row_number() OVER (ORDER BY queued_at, id) AS n FROM tool_runs
  WHERE tool='ci' AND work BETWEEN 238 AND 268 AND ended_at IS NOT NULL AND queued_at < 1793827133000)
SELECT ci.n AS ci_calls, datetime(ci.q/1000,'unixepoch') AS at_utc,
       round(sum(CASE WHEN t.tool='ci' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS ci_min,
       round(sum(CASE WHEN t.tool='ci_before' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS before_min,
       round(sum(CASE WHEN t.tool='full_ci' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS full_min,
       round(sum(CASE WHEN t.tool='merge' THEN t.ended_at-t.started_at ELSE 0 END)/60000.0,1) AS merge_min,
       round(sum(t.ended_at-t.started_at)/60000.0,1) AS total_min,
       round(sum(t.ended_at-t.started_at)/60000.0/25,1) AS per_work_min
FROM ci JOIN tool_runs t
  ON t.work BETWEEN 238 AND 268 AND t.queued_at <= ci.q AND t.ended_at IS NOT NULL
 AND t.tool IN ('ci','ci_before','full_ci','merge') AND t.work IN (SELECT work FROM tool_runs WHERE tool='ci')
WHERE ci.n BETWEEN 247 AND 250
GROUP BY ci.n ORDER BY ci.n;

-- Q8. Was the first full CI of a work green? The first row of a work among the tool `full_ci` (result field `first`
-- holds the checks) and the tool `merge` (its result is the list of checks); green when no check is red or cancelled.
-- By the moment of that row, and for works 238 to 268.
WITH f AS (
  SELECT id, work, queued_at AS q, tool,
         CASE WHEN tool='full_ci' THEN json_extract(result,'$.first') ELSE result END AS chk
  FROM tool_runs
  WHERE queued_at < 1793827133000
    AND ((tool='full_ci' AND json_type(result,'$.first')='array') OR (tool='merge' AND substr(result,1,2)='[{'))),
fst AS (SELECT work, min(id) AS id FROM f GROUP BY work),
g AS (SELECT f.*, (instr(f.chk,'"result":"red"')=0 AND instr(f.chk,'"result":"cancelled"')=0) AS green
      FROM f JOIN fst USING(id))
SELECT CASE WHEN q < 1793703148000 THEN 'before the switch' ELSE 'after the switch' END AS period, tool,
       count(*) AS works, sum(green) AS first_green
FROM g GROUP BY 1, 2
UNION ALL SELECT 'works 238-268', tool, count(*), sum(green) FROM g WHERE work BETWEEN 238 AND 268 GROUP BY 2;

-- Q9. How many `ci` calls had a red check, whole CI and named tests, up to the cutoff.
SELECT CASE WHEN json_extract(result,'$.push.run') IS NULL THEN 'whole CI' ELSE 'named tests' END AS regime,
       count(*) AS n, sum(instr(result,'"result":"red"')>0) AS red
FROM tool_runs
WHERE tool='ci' AND ended_at-started_at >= 10000 AND json_extract(result,'$.push.verdict')='pushed'
  AND queued_at < 1793827133000
GROUP BY 1;

-- Q10. The sample of runs whose job times were read from GitHub. For `ci`: every 4th call (by id) of those with a test
-- job, apart for the calls whose round also asked for ci_before (`together` 1) and those whose round did not. For
-- `ci_before`: all calls whose round did not ask for ci (11), every 6th of the others. t0 and t1 are the tool run's
-- start and end in epoch seconds; run and job are GitHub's numbers, cut out of the check's link. The sample was taken
-- at about 21:10 UTC on 2026-11-04 and has no cutoff, so a re-run adds rows at the end; the rows it gave are in
-- ci-speed-measured.sql, which is what the report uses.
WITH s AS (
  SELECT t.id, t.work, t.round, t.started_at/1000 AS t0, t.ended_at/1000 AS t1, json_extract(t.result,'$.push.run') AS kind,
         (SELECT json_extract(value,'$.run') FROM json_each(t.result,'$.push.checks') WHERE json_extract(value,'$.name')='test') AS u,
         EXISTS(SELECT 1 FROM tool_runs o WHERE o.work=t.work AND o.round=t.round AND o.tool='ci_before') AS together
  FROM tool_runs t
  WHERE t.tool='ci' AND t.ended_at IS NOT NULL AND t.ended_at-t.started_at >= 10000 AND t.started_at >= 1793703148000
    AND json_extract(t.result,'$.push.verdict')='pushed'),
n AS (SELECT *, row_number() OVER (PARTITION BY together ORDER BY id) AS rn FROM s WHERE u IS NOT NULL)
SELECT id, work, round, t0, t1, kind, together,
       substr(u, instr(u,'/runs/')+6, instr(u,'/job/')-instr(u,'/runs/')-6) AS run, substr(u, instr(u,'/job/')+5) AS job
FROM n WHERE rn % 4 = 0 ORDER BY id;

WITH s AS (
  SELECT t.id, t.work, t.round, t.started_at/1000 AS t0, t.ended_at/1000 AS t1,
         (SELECT json_extract(value,'$.run') FROM json_each(t.result,'$.ran.checks') WHERE json_extract(value,'$.name')='test') AS u,
         EXISTS(SELECT 1 FROM tool_runs o WHERE o.work=t.work AND o.round=t.round AND o.tool='ci') AS together
  FROM tool_runs t
  WHERE t.tool='ci_before' AND t.ended_at IS NOT NULL AND t.ended_at-t.started_at >= 10000 AND t.started_at >= 1793703148000
    AND json_extract(t.result,'$.ran.verdict')='ran'),
n AS (SELECT *, row_number() OVER (PARTITION BY together ORDER BY id) AS rn FROM s WHERE u IS NOT NULL)
SELECT id, work, round, t0, t1, together,
       substr(u, instr(u,'/runs/')+6, instr(u,'/job/')-instr(u,'/runs/')-6) AS run, substr(u, instr(u,'/job/')+5) AS job
FROM n WHERE together = 0 OR rn % 6 = 0 ORDER BY id;
