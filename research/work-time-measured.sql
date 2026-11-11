-- The GitHub job times behind research/work-time-measured.md (runner wait), and the SQL that sums them per work.
-- It is one SELECT, so that it runs as one statement of the engine's `query` tool (how the report's figures were got,
-- with these comments left out) and with `sqlite3 -safe -readonly :memory: < research/work-time-measured.sql` (not
-- tried). It reads nothing but the tables of data below.
--
-- Whom it serves: a reader who wants to re-check the report's runner-wait numbers without GitHub (the job records of a
-- run are kept for 90 days only, so what GitHub gave on 2026-11-05 is kept here). It is removed with the report.
--
-- m  one row for each `ci`, `ci_before` or `merge` call of the six works of the group "after" (the report's section 2)
--    whose result names a GitHub run: the engine's tool_runs id, the work, the round, the tool, the call's start and end
--    in epoch seconds, and the run's number. Written by query Q8 of research/work-time-measured-queries.sql, cut off at
--    2026-11-05 13:30:27 UTC.
-- j  one row for each of those runs (and for three runs of works outside the group, see below), written by
--    `gh api repos/<repo>/actions/runs/<run>/jobs` on 2026-11-05: the jobs sorted by name, and for each job
--      q  the seconds from the job's created_at to its started_at (the wait for a runner),
--      d  the seconds from its started_at to its completed_at (-1 for a skipped job; read as 0).
--    Columns: run, njobs, on14900k (1 when the last job by name, the test job, has the label 14900k), c0 (created_at of
--    the first job by name, in epoch seconds), then q0, d0, q1, d1, ... of the jobs in name order. For a run of three
--    jobs the first by name (`fast`, or `check` of the full CI) only waits for the other two and is the run's last job;
--    `lint` and `test` start together when the run starts. The full CI of a merge may have five jobs, the first by name
--    is `check`; with two jobs, all are taken as starting together.
-- pw the minutes of query Q2 at the same cutoff for the six works (rounded to 0.01 min).
--
-- The runs 41935104426, 41935012165 and 41936837070 are not in m: they are the three runs read to show that works 324,
-- 323 and 308 ran some of their CI on the hosted ubuntu-24.04 runners (on14900k 0), so that they are not in "after".
--
-- Output, one section after another: `runs`, `per work`, `category`, `residual`, `call`, `round kind`.

WITH m(tr, work, round, tool, t0, t1, run) AS (VALUES
(5443,328,15,'ci',1793838297,1793838385,41941160564),
(5465,317,38,'ci',1793838390,1793838535,41941352404),
(5475,328,17,'ci',1793838469,1793838745,41941504678),
(5476,328,17,'ci_before',1793838470,1793838586,41941506759),
(5478,317,40,'ci',1793838587,1793838705,41941745815),
(5486,328,19,'ci',1793838766,1793838844,41942127592),
(5495,328,21,'ci',1793838863,1793838945,41942329184),
(5509,328,24,'ci_before',1793838994,1793839059,41942601771),
(5536,330,7,'ci',1793839397,1793839479,41943433826),
(5537,330,7,'ci_before',1793839399,1793839469,41943430801),
(5557,328,29,'ci',1793839474,1793839559,41943581813),
(5558,317,53,'ci',1793839476,1793839507,41943584934),
(5559,328,29,'ci_before',1793839476,1793839547,41943583784),
(5560,330,9,'ci',1793839511,1793839658,41943659747),
(5573,328,31,'ci',1793839642,1793839737,41943925774),
(5574,328,31,'ci_before',1793839644,1793839711,41943927481),
(5581,328,34,'ci_before',1793839799,1793839880,41944238992),
(5582,328,34,'ci_before',1793839799,1793839884,41944239374),
(5583,328,34,'ci',1793839799,1793839919,41944239518),
(5589,330,14,'ci',1793839950,1793840175,41944546385),
(5590,330,14,'ci_before',1793839952,1793840157,41944547604),
(5591,328,37,'ci',1793840026,1793840161,41944702000),
(5610,328,39,'ci',1793840174,1793840261,41944998024),
(5611,330,16,'ci',1793840190,1793840332,41945029000),
(5612,330,16,'ci_before',1793840192,1793840288,41945031636),
(5626,328,42,'ci',1793840306,1793840363,41945245945),
(5628,331,11,'ci',1793840342,1793840507,41945319630),
(5629,330,20,'ci_before',1793840350,1793840510,41945328169),
(5630,330,20,'ci',1793840350,1793840512,41945328867),
(5632,328,45,'merge',1793840372,1793840513,41945370339),
(5664,330,24,'ci_before',1793840530,1793840580,41945693416),
(5665,330,24,'ci',1793840530,1793840587,41945693598),
(5670,330,26,'ci_before',1793840603,1793840679,41945835636),
(5676,331,18,'ci',1793840696,1793840734,41946023222),
(5680,330,29,'ci_before',1793840729,1793840894,41946087110),
(5681,330,29,'ci',1793840729,1793840885,41946087819),
(5683,331,22,'merge',1793840755,1793840955,41946142232),
(5702,330,33,'merge',1793840963,1793841087,41946554499),
(5716,330,35,'ci',1793841105,1793841155,41946827249),
(5732,330,39,'merge',1793841174,1793841177,41946554499),
(5736,330,45,'merge',1793841209,1793841212,41946554499),
(5746,332,5,'ci',1793841349,1793841452,41947311708),
(5747,332,5,'ci_before',1793841350,1793841410,41947309792),
(5757,332,9,'ci_before',1793841471,1793841504,41947545031),
(5758,332,9,'ci',1793841471,1793841503,41947546347),
(5759,332,11,'ci_before',1793841527,1793841568,41947655187),
(5760,332,11,'ci',1793841527,1793841568,41947654173),
(5765,332,20,'merge',1793841674,1793841744,41947941871),
(5779,330,47,'merge',1793841962,1793842058,41948494863),
(5811,317,60,'merge',1793842358,1793842479,41949231920),
(5943,334,14,'ci',1793843665,1793843704,41951647524),
(5961,334,21,'ci',1793843816,1793843854,41951918327),
(5965,334,25,'merge',1793843872,1793843979,41952011223)),
j(run, njobs, on14900k, c0, q0, d0, q1, d1, q2, d2, q3, d3, q4, d4) AS (VALUES
(41941160564,3,1,1793838371,2,3,2,64,6,38,NULL,NULL,NULL,NULL),
(41941352404,3,1,1793838437,87,3,0,0,1,38,NULL,NULL,NULL,NULL),
(41941504678,3,1,1793838658,80,4,115,69,117,45,NULL,NULL,NULL,NULL),
(41941506759,3,1,1793838502,62,14,0,0,16,11,NULL,NULL,NULL,NULL),
(41941745815,3,1,1793838639,60,3,0,0,2,43,NULL,NULL,NULL,NULL),
(41942127592,3,1,1793838834,2,4,2,60,2,20,NULL,NULL,NULL,NULL),
(41942329184,3,1,1793838936,2,4,2,66,2,15,NULL,NULL,NULL,NULL),
(41942601771,3,1,1793839044,2,4,0,-1,2,44,NULL,NULL,NULL,NULL),
(41943433826,3,1,1793839465,2,4,2,59,1,46,NULL,NULL,NULL,NULL),
(41943430801,3,1,1793839452,2,3,0,-1,2,46,NULL,NULL,NULL,NULL),
(41943581813,3,1,1793839545,2,5,2,64,2,39,NULL,NULL,NULL,NULL),
(41943584934,3,1,1793839497,2,4,0,-1,2,11,NULL,NULL,NULL,NULL),
(41943583784,3,1,1793839526,7,3,0,0,2,44,NULL,NULL,NULL,NULL),
(41943659747,3,1,1793839646,2,4,65,64,2,38,NULL,NULL,NULL,NULL),
(41943925774,3,1,1793839722,2,4,7,68,2,43,NULL,NULL,NULL,NULL),
(41943927481,3,1,1793839698,2,4,0,0,8,42,NULL,NULL,NULL,NULL),
(41944238992,3,1,1793839817,49,4,0,0,2,11,NULL,NULL,NULL,NULL),
(41944239374,3,1,1793839864,7,4,0,0,2,58,NULL,NULL,NULL,NULL),
(41944239518,3,1,1793839892,14,4,2,86,2,56,NULL,NULL,NULL,NULL),
(41944546385,3,1,1793840074,88,4,0,-1,68,51,NULL,NULL,NULL,NULL),
(41944547604,3,1,1793840114,31,4,0,0,114,43,NULL,NULL,NULL,NULL),
(41944702000,3,1,1793840144,6,4,49,62,45,44,NULL,NULL,NULL,NULL),
(41944998024,3,1,1793840253,1,4,7,65,14,43,NULL,NULL,NULL,NULL),
(41945029000,3,1,1793840319,2,4,0,-1,85,39,NULL,NULL,NULL,NULL),
(41945031636,3,1,1793840272,2,4,0,0,40,35,NULL,NULL,NULL,NULL),
(41945245945,3,1,1793840352,2,4,0,-1,2,39,NULL,NULL,NULL,NULL),
(41945319630,3,1,1793840389,111,3,0,-1,2,37,NULL,NULL,NULL,NULL),
(41945328169,3,1,1793840394,100,4,0,-1,2,38,NULL,NULL,NULL,NULL),
(41945328867,3,1,1793840399,106,3,0,0,2,42,NULL,NULL,NULL,NULL),
(41945370339,5,1,1793840503,2,4,1,117,17,108,23,92,13,29),
(41945693416,3,1,1793840567,2,3,0,-1,2,30,NULL,NULL,NULL,NULL),
(41945693598,3,1,1793840575,2,4,0,0,2,38,NULL,NULL,NULL,NULL),
(41945835636,3,1,1793840665,2,4,0,0,2,55,NULL,NULL,NULL,NULL),
(41946023222,3,1,1793840724,2,3,0,-1,2,19,NULL,NULL,NULL,NULL),
(41946087110,3,1,1793840788,92,4,0,0,12,41,NULL,NULL,NULL,NULL),
(41946087819,3,1,1793840781,95,4,0,-1,2,46,NULL,NULL,NULL,NULL),
(41946142232,3,1,1793840878,67,4,22,92,23,95,NULL,NULL,NULL,NULL),
(41946554499,3,1,1793841073,2,4,1,86,2,102,NULL,NULL,NULL,NULL),
(41946827249,3,1,1793841142,2,4,2,26,2,17,NULL,NULL,NULL,NULL),
(41947311708,3,1,1793841442,2,3,1,47,2,84,NULL,NULL,NULL,NULL),
(41947309792,3,1,1793841396,2,4,0,0,2,39,NULL,NULL,NULL,NULL),
(41947545031,3,1,1793841490,2,4,0,0,2,12,NULL,NULL,NULL,NULL),
(41947546347,3,1,1793841491,2,4,0,-1,2,13,NULL,NULL,NULL,NULL),
(41947655187,3,1,1793841552,2,4,0,-1,2,18,NULL,NULL,NULL,NULL),
(41947654173,3,1,1793841558,2,4,0,-1,1,25,NULL,NULL,NULL,NULL),
(41947941871,3,1,1793841734,2,3,2,40,1,54,NULL,NULL,NULL,NULL),
(41948494863,3,1,1793842048,2,3,2,53,2,78,NULL,NULL,NULL,NULL),
(41949231920,3,1,1793842470,2,4,2,30,1,106,NULL,NULL,NULL,NULL),
(41951647524,2,1,1793843670,2,30,0,0,NULL,NULL,NULL,NULL,NULL,NULL),
(41951918327,2,1,1793843822,2,28,0,0,NULL,NULL,NULL,NULL,NULL,NULL),
(41952011223,3,1,1793843970,3,4,1,26,2,93,NULL,NULL,NULL,NULL),
(41935104426,3,0,1793835579,3,3,2,133,3,163,NULL,NULL,NULL,NULL),
(41935012165,3,0,1793835542,2,3,3,158,3,169,NULL,NULL,NULL,NULL),
(41936837070,3,0,1793836444,10,3,3,125,3,215,NULL,NULL,NULL,NULL)),
pw(work, span, agent, ci_tools, merge, light_tools, wait_ask, engine, gap_slot, gap_other) AS (VALUES
(317,253.35,101.63,5.92,2.17,0.17,0.0,0.04,42.99,100.44),
(328,52.62,29.01,19.5,2.49,0.01,0.0,0.02,0.0,1.58),
(330,55.36,18.86,18.47,3.96,0.03,12.44,0.03,0.01,1.57),
(331,26.17,18.9,3.37,3.45,0.02,0.0,0.03,0.0,0.4),
(332,11.5,6.72,3.02,1.29,0.02,0.0,0.03,0.0,0.43),
(334,14.93,9.67,2.78,1.91,0.02,0.0,0.03,0.0,0.52)),
-- The jobs of a run that start together when the run starts, with their wait q and run time d.
leaf AS (
  SELECT run, 1 AS k, q1 AS q, max(d1, 0) AS d FROM j WHERE njobs >= 3
  UNION ALL SELECT run, 2, q2, max(d2, 0) FROM j WHERE njobs >= 3
  UNION ALL SELECT run, 3, q3, max(d3, 0) FROM j WHERE njobs >= 3 AND q3 IS NOT NULL
  UNION ALL SELECT run, 4, q4, max(d4, 0) FROM j WHERE njobs >= 3 AND q4 IS NOT NULL
  UNION ALL SELECT run, 0, q0, max(d0, 0) FROM j WHERE njobs < 3
  UNION ALL SELECT run, 1, q1, max(d1, 0) FROM j WHERE njobs < 3),
-- The runner wait on a run's critical path, in seconds: the wait of the run's last job, plus the wait of the job among
-- the others that ended last (its wait q plus its run time d, all started together).
crit AS (
  SELECT j.run, j.on14900k, (CASE WHEN j.njobs >= 3 THEN j.q0 ELSE 0 END) + c.q AS queue_s
  FROM j JOIN (SELECT run, q, row_number() OVER (PARTITION BY run ORDER BY q + d DESC, k) AS rn FROM leaf) c
    ON c.run = j.run AND c.rn = 1),
-- Each round's runner wait: the call that lasted longest in the round, its run's critical-path wait, and only for the
-- first call that used the run (a merge that looks at the same run again waits for nothing).
pick AS (
  SELECT m.work, m.round, m.tool, m.tr, m.t1 - m.t0 AS secs, crit.queue_s
  FROM m JOIN crit ON crit.run = m.run
  WHERE m.tr = (SELECT x.tr FROM m x WHERE x.work = m.work AND x.round = m.round ORDER BY x.t1 - x.t0 DESC, x.tr LIMIT 1)
    AND m.tr = (SELECT min(y.tr) FROM m y WHERE y.run = m.run)),
-- Runner wait per work in minutes, apart for the rounds that ran ci or ci_before and for the merge rounds.
pq AS (
  SELECT pw.work, round(coalesce(sum(CASE WHEN pick.tool != 'merge' THEN pick.queue_s END), 0) / 60.0, 2) AS q_ci,
         round(coalesce(sum(CASE WHEN pick.tool = 'merge' THEN pick.queue_s END), 0) / 60.0, 2) AS q_merge
  FROM pw LEFT JOIN pick ON pick.work = pw.work GROUP BY pw.work),
-- The categories of the report's section 3, exclusive, adding up to the span. `ci rounds` and `merge round` are the
-- engine's rounds that ran ci, ci_before or merge, less the runner wait in them.
cat AS (
  SELECT pw.work, (pw.work != 317) AS small, 'span' AS m, pw.span AS x FROM pw
  UNION ALL SELECT pw.work, pw.work != 317, 'agent rounds', pw.agent FROM pw
  UNION ALL SELECT pw.work, pw.work != 317, 'ci rounds, less runner wait', pw.ci_tools - pq.q_ci FROM pw JOIN pq USING (work)
  UNION ALL SELECT pw.work, pw.work != 317, 'merge round, less runner wait', pw.merge - pq.q_merge FROM pw JOIN pq USING (work)
  UNION ALL SELECT pw.work, pw.work != 317, 'runner wait', pq.q_ci + pq.q_merge FROM pw JOIN pq USING (work)
  UNION ALL SELECT pw.work, pw.work != 317, 'waiting for an answer (A2, A3)', pw.wait_ask FROM pw
  UNION ALL SELECT pw.work, pw.work != 317, 'gap: waiting for a slot', pw.gap_slot FROM pw
  UNION ALL SELECT pw.work, pw.work != 317, 'gap: other', pw.gap_other FROM pw
  UNION ALL SELECT pw.work, pw.work != 317, 'engine steps and light tools', pw.engine + pw.light_tools FROM pw),
sc AS (
  SELECT 'after, all' AS scope, m, x FROM cat
  UNION ALL SELECT 'after, no deep research', m, x FROM cat WHERE small = 1),
r AS (
  SELECT scope, m, x, row_number() OVER (PARTITION BY scope, m ORDER BY x) AS rn, count(*) OVER (PARTITION BY scope, m) AS n FROM sc),
a AS (
  SELECT scope, m, count(*) AS n, avg(CASE WHEN rn IN ((n + 1) / 2, (n + 2) / 2) THEN x END) AS med, sum(x) AS total FROM r GROUP BY scope, m),
-- The longest call of each round, numbered inside its kind (merge, or ci and ci_before) for the median.
pr AS (
  SELECT tool, secs, queue_s, (tool != 'merge') AS is_ci,
         row_number() OVER (PARTITION BY (tool != 'merge') ORDER BY secs) AS rn,
         count(*) OVER (PARTITION BY (tool != 'merge')) AS n FROM pick)
SELECT section, a, b, c, d, e, f, g FROM (
  SELECT 'runs' AS section, 'every run of the group read' AS a, count(*) AS b, sum(on14900k) AS c, NULL AS d, NULL AS e, NULL AS f, NULL AS g
    FROM crit WHERE run NOT IN (41935104426, 41935012165, 41936837070)
  UNION ALL SELECT 'runs', 'the three runs read outside the group', count(*), sum(on14900k), NULL, NULL, NULL, NULL
    FROM crit WHERE run IN (41935104426, 41935012165, 41936837070)
  -- per work, minutes: a work; b runner wait in ci rounds; c ci rounds; d runner wait in the merge round; e merge round;
  -- f runner wait in all
  UNION ALL SELECT 'per work', pw.work, pq.q_ci, pw.ci_tools, pq.q_merge, pw.merge, round(pq.q_ci + pq.q_merge, 2), NULL
    FROM pw JOIN pq ON pq.work = pw.work
  -- category: a scope and category; b n; c median minutes; d sum minutes; e share of the sum of the spans in percent
  UNION ALL SELECT 'category', scope || ' / ' || m, n, round(med, 2), round(total, 1),
         round(100.0 * total / (SELECT a2.total FROM a a2 WHERE a2.scope = a.scope AND a2.m = 'span'), 1), NULL, NULL
    FROM a
  -- residual: a work; b the span less the categories in minutes (the inputs are rounded to 0.01 min)
  UNION ALL SELECT 'residual', work, round(max(CASE WHEN m = 'span' THEN x END) - sum(CASE WHEN m != 'span' THEN x END), 2), NULL, NULL, NULL, NULL, NULL
    FROM cat GROUP BY work
  -- call: the longest call of each round; a kind; b calls; c their seconds; d their runner wait seconds; e wait share
  -- of the calls' time in percent; f median seconds of a call; g longest wait in seconds
  UNION ALL SELECT 'call', CASE is_ci WHEN 1 THEN 'ci or ci_before' ELSE 'merge' END, count(*), sum(secs), sum(queue_s),
         round(100.0 * sum(queue_s) / sum(secs), 1), avg(CASE WHEN rn IN ((n + 1) / 2, (n + 2) / 2) THEN secs END), max(queue_s)
    FROM pr GROUP BY is_ci
  -- round kind: ci and ci_before asked in the same round, or alone; b rounds; c mean wait seconds; d longest wait seconds;
  -- e mean call seconds
  UNION ALL SELECT 'round kind',
         CASE WHEN EXISTS (SELECT 1 FROM m x WHERE x.work = pick.work AND x.round = pick.round AND x.tool != pick.tool AND x.tool IN ('ci', 'ci_before'))
              THEN 'ci and ci_before in the round' ELSE 'alone' END,
         count(*), round(avg(queue_s), 1), max(queue_s), round(avg(secs), 1), NULL, NULL
    FROM pick WHERE tool IN ('ci', 'ci_before') GROUP BY 2
) ORDER BY section, a;
