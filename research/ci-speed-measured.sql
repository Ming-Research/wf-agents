-- The sample behind research/ci-speed-measured.md, section "Where one ci call spends its time", and the SQL that
-- summarises it. Run it with:
--
--   sqlite3 -safe -readonly :memory: < research/ci-speed-measured.sql
--
-- Whom it serves: a reader who wants to re-check the report's segment tables without GitHub (the job records of a run
-- are kept for 90 days only, so the numbers GitHub gave are kept here). It is removed when the report is.
--
-- One row is one run of .github/workflows/fast.yml that the engine's tool `ci` or `ci_before` waited for (rows picked
-- by the query in the report's appendix: every 4th `ci` call with a test job, apart for the calls with and without a
-- `ci_before` in the same round, and all `ci_before` calls without a `ci`, every 6th of those with one). Times are
-- UTC epoch seconds, durations seconds, all read from `gh api repos/<repo>/actions/runs/<run>/jobs`:
--   t0, t1    the engine's tool run started and ended (tool_runs.started_at, ended_at)
--   created   the first job of the run was created; e the last job completed
--   qtest     the test job waited for a runner
--   setup     steps from "Set up job" to setup-node, but not the cache restore
--   cache     the step "actions/cache/restore" (the incremental build state of main); 0 or 1 s is a miss
--   page      the step "Build the page" (npm ci and vite build)
--   test      the step "Test" (cargo nextest: compile and run), tpage the step "Test the page"
--   tjob      the whole test job, from its start to its end
--   qlint     the lint job waited for a runner (0 when it was skipped)
--   lintvs    the lint job's end less the test job's end (positive: lint ended later)
--   tail      the run's end less the test job's end
--   concl     the test job's conclusion
CREATE TEMP TABLE s (
  tool TEXT, together INTEGER, kind TEXT, t0 INTEGER, t1 INTEGER, run INTEGER, created INTEGER, e INTEGER,
  qtest INTEGER, setup INTEGER, cache INTEGER, page INTEGER, test INTEGER, tpage INTEGER, tjob INTEGER,
  qlint INTEGER, lintvs INTEGER, tail INTEGER, concl TEXT);
INSERT INTO s VALUES
('ci',1,'named_and_lint',1793704492,1793704691,41625957548,1793704498,1793704665,3,25,21,76,32,0,158,2,-30,6,'failure'),
('ci',0,'named_and_lint',1793705282,1793705479,41627749234,1793705288,1793705462,2,23,16,75,37,0,155,2,-24,17,'success'),
('ci',0,'named_and_lint',1793707153,1793707332,41631927100,1793707159,1793707314,2,22,24,52,41,0,142,2,5,11,'failure'),
('ci',1,'named_and_lint',1793707515,1793707673,41632768337,1793707520,1793707661,3,22,19,49,40,0,133,3,-1,5,'failure'),
('ci',0,'named_and_lint',1793709242,1793709423,41636810772,1793709250,1793709403,3,23,17,74,29,0,145,3,-100,5,'failure'),
('ci',1,'named_and_lint',1793709521,1793709738,41637459871,1793709525,1793709723,3,24,24,34,37,0,123,3,67,72,'success'),
('ci',0,'named_and_lint',1793710532,1793710735,41639829640,1793710540,1793710710,3,21,0,50,86,0,160,3,-38,7,'failure'),
('ci',1,'named_and_lint',1793712519,1793712758,41644670712,1793712525,1793712734,2,21,0,74,70,0,168,2,32,39,'success'),
('ci',0,'named_and_lint',1793712529,1793712751,41644704236,1793712539,1793712741,3,22,16,47,42,0,130,3,63,69,'success'),
('ci',0,'named_and_lint',1793714431,1793714615,41649504792,1793714440,1793714599,3,25,22,76,22,0,149,3,-124,7,'failure'),
('ci',0,'named_and_lint',1793714943,1793715208,41650866906,1793714951,1793715192,3,25,28,65,110,0,232,3,-60,6,'failure'),
('ci',0,'named_and_lint',1793715935,1793716139,41653582580,1793715945,1793716118,2,20,26,32,83,0,165,2,-140,6,'success'),
('ci',0,'named_and_lint',1793716741,1793716960,41655791993,1793716747,1793716944,3,28,28,62,42,0,165,3,21,29,'success'),
('ci',1,'named_and_lint',1793717429,1793717692,41657682096,1793717435,1793717668,3,28,20,76,87,0,218,3,-43,12,'failure'),
('ci',0,'named_and_lint',1793718031,1793718275,41659379529,1793718041,1793718248,8,25,24,41,13,0,105,3,87,94,'success'),
('ci',1,'named_and_lint',1793718642,1793718781,41661158889,1793718650,1793718763,3,21,0,33,47,0,104,3,-73,6,'success'),
('ci',0,'named_and_lint',1793718892,1793719116,41661897930,1793718898,1793719087,4,21,0,51,96,0,179,4,-145,6,'success'),
('ci',1,'named_and_lint',1793720245,1793720464,41665814402,1793720254,1793720456,2,24,19,49,57,0,154,2,41,46,'success'),
('ci',0,'named_and_lint',1793720735,1793720967,41667211006,1793720742,1793720914,8,25,17,71,39,0,158,8,-125,6,'success'),
('ci',0,'named_and_lint',1793722176,1793722398,41671300858,1793722183,1793722375,3,25,1,73,77,0,181,4,-150,8,'success'),
('ci',1,'named_and_lint',1793723483,1793723664,41675071658,1793723490,1793723650,3,27,20,49,44,0,144,4,-115,13,'success'),
('ci',0,'named_and_lint',1793723895,1793724114,41676240805,1793723901,1793724097,3,31,26,64,49,0,173,2,13,20,'success'),
('ci',0,'named_and_lint',1793724813,1793725052,41678793286,1793724819,1793725039,8,49,18,48,42,0,163,3,42,48,'success'),
('ci',1,'named_and_lint',1793725072,1793725310,41679502029,1793725077,1793725291,3,22,14,50,44,0,134,3,71,77,'success'),
('ci',0,'named_and_lint',1793725378,1793725599,41680356752,1793725385,1793725577,3,27,20,81,43,0,183,2,-15,6,'failure'),
('ci',0,'named_and_lint',1793726598,1793726837,41683764645,1793726603,1793726812,2,23,17,79,76,0,199,3,-30,7,'success'),
('ci',0,'named_and_lint',1793727734,1793727937,41686809836,1793727743,1793727925,3,28,28,41,50,0,151,6,23,28,'success'),
('ci',0,'named_and_lint',1793729073,1793729296,41690314817,1793729081,1793729280,2,32,29,63,63,0,190,3,-43,7,'failure'),
('ci',1,'named_and_lint',1793730412,1793730656,41693908458,1793730419,1793730642,2,34,22,77,79,0,215,2,-180,6,'failure'),
('ci',0,'named_and_lint',1793730653,1793730856,41694551436,1793730662,1793730843,3,37,21,75,34,0,170,2,-115,8,'failure'),
('ci',0,'named_and_lint',1793731710,1793731928,41697323625,1793731716,1793731914,2,37,20,81,47,0,190,2,-74,6,'success'),
('ci',1,'named_and_checks',1793811260,1793811505,41872206702,1793811270,1793811481,3,27,31,75,63,0,202,2,-175,6,'success'),
('ci',0,'named_and_checks',1793811583,1793811763,41873111514,1793811595,1793811746,3,26,49,33,17,0,131,3,-7,17,'success'),
('ci',1,'named',1793820925,1793821140,41898410279,1793820931,1793821120,2,24,26,86,41,0,181,0,-183,6,'success'),
('ci',0,'named_and_checks',1793821568,1793821810,41900142312,1793821575,1793821783,3,26,35,87,42,0,197,3,-166,8,'success'),
('ci',1,'named',1793823310,1793823528,41904830571,1793823316,1793823516,2,24,54,72,36,0,191,0,-194,7,'failure'),
('ci',1,'named_and_checks',1793824855,1793825094,41908911498,1793824861,1793825070,3,26,0,87,76,5,200,2,-103,6,'success'),
('ci',0,'named_and_checks',1793825135,1793825375,41909651820,1793825141,1793825347,3,25,0,91,72,5,197,2,-66,6,'success'),
('ci_before',0,'named',1793704089,1793704228,41625016332,1793704094,1793704214,3,25,19,48,14,0,110,0,-113,7,'failure'),
('ci_before',1,'named',1793706364,1793706480,41630175830,1793706369,1793706468,2,18,22,42,4,0,89,0,-91,8,'failure'),
('ci_before',1,'named',1793707911,1793708113,41633690799,1793707919,1793708088,3,28,27,71,32,0,161,0,-164,5,'failure'),
('ci_before',1,'named',1793711370,1793711508,41641844691,1793711376,1793711484,3,19,27,50,0,0,98,0,-101,7,'failure'),
('ci_before',0,'named',1793711817,1793711956,41642960727,1793711823,1793711930,3,30,26,38,0,0,97,0,-100,7,'failure'),
('ci_before',0,'named',1793714126,1793714288,41648723962,1793714133,1793714262,3,23,16,78,0,0,120,0,-123,6,'failure'),
('ci_before',0,'named',1793714127,1793714306,41648723228,1793714133,1793714288,3,20,15,77,31,0,147,0,-150,5,'success'),
('ci_before',0,'named',1793714843,1793714960,41650594023,1793714847,1793714947,3,24,24,38,0,0,90,0,-93,7,'failure'),
('ci_before',1,'named',1793715290,1793715449,41651804251,1793715296,1793715422,4,26,20,51,14,0,115,0,-119,7,'failure'),
('ci_before',1,'named',1793718325,1793718489,41660248100,1793718338,1793718478,3,26,23,49,27,0,130,0,-134,7,'success'),
('ci_before',0,'named',1793718571,1793718729,41660946114,1793718577,1793718705,4,21,26,53,11,0,114,0,-118,10,'success'),
('ci_before',1,'named',1793718934,1793719073,41662018294,1793718941,1793719059,2,22,17,48,17,0,107,0,-109,9,'failure'),
('ci_before',0,'named',1793721476,1793721658,41669281974,1793721482,1793721627,3,25,19,76,14,0,136,0,-140,6,'failure'),
('ci_before',0,'named',1793722153,1793722312,41671228993,1793722158,1793722294,3,27,25,57,14,0,126,0,-130,7,'failure'),
('ci_before',1,'named',1793722734,1793722912,41672922802,1793722739,1793722884,3,25,15,73,20,0,136,0,-139,6,'success'),
('ci_before',0,'named',1793724486,1793724645,41677895076,1793724492,1793724617,9,26,26,50,6,0,111,0,-120,5,'failure'),
('ci_before',1,'named',1793727964,1793728119,41687411988,1793727969,1793728097,2,29,21,47,18,0,119,0,-122,7,'success'),
('ci_before',1,'named',1793731378,1793731537,41696447251,1793731383,1793731516,3,36,28,32,25,0,124,0,-127,6,'failure'),
('ci_before',0,'named',1793731960,1793732117,41697973335,1793731965,1793732094,3,22,37,34,25,0,121,0,-124,5,'failure'),
('ci_before',1,'named',1793821453,1793821651,41899820698,1793821458,1793821625,2,21,27,87,20,0,159,0,-161,6,'failure'),
('ci_before',0,'named',1793822286,1793822464,41902040926,1793822291,1793822448,3,22,25,86,11,0,147,0,-150,7,'failure'),
('ci_before',1,'named',1793824629,1793824807,41908325511,1793824634,1793824795,2,22,26,86,12,2,152,0,-154,7,'failure');

-- Each run cut into the segments that follow one another from the engine's start to its end:
--   pre + qtest + tjob + tail + post = total, with tjob = setup + cache + page + test + tpage + other, and
--   tail = lint_wait + summary (the time the run lasted after the test job ended).
CREATE TEMP VIEW d AS SELECT tool, together, kind, run, created,
  created - t0 AS pre, qtest, setup, cache, page, test, tpage,
  tjob - (setup + cache + page + test + tpage) AS other, tjob,
  max(0, lintvs) AS lint_wait, tail - max(0, lintvs) AS summary,
  t1 - e AS post, e - created AS gh_run, t1 - t0 AS total,
  (pre_check) AS residual, cache >= 10 AS hit, CASE WHEN qlint > 0 THEN qlint END AS qlint
FROM (SELECT s.*, (t1 - t0) - ((created - t0) + qtest + tjob + tail + (t1 - e)) AS pre_check FROM s);

-- 1. The segments, by group: n, mean, median (the mean of the two middle values for an even n), min and max.
CREATE TEMP VIEW dd AS
  SELECT 'ci, all' AS grp, * FROM d WHERE tool = 'ci'
  UNION ALL SELECT 'ci, a ci_before in the round', * FROM d WHERE tool = 'ci' AND together = 1
  UNION ALL SELECT 'ci, no ci_before in the round', * FROM d WHERE tool = 'ci' AND together = 0
  UNION ALL SELECT 'ci_before, all', * FROM d WHERE tool = 'ci_before'
  UNION ALL SELECT 'ci_before, a ci in the round', * FROM d WHERE tool = 'ci_before' AND together = 1
  UNION ALL SELECT 'ci_before, no ci in the round', * FROM d WHERE tool = 'ci_before' AND together = 0;
CREATE TEMP VIEW m AS
  SELECT grp, 1 AS o, 'pre (engine, before the run exists)' AS metric, pre AS x FROM dd
  UNION ALL SELECT grp, 2, 'qtest (waiting for a runner)', qtest FROM dd
  UNION ALL SELECT grp, 3, 'setup (job, checkout, toolchain, caches but main state)', setup FROM dd
  UNION ALL SELECT grp, 4, 'cache (restore of main build state)', cache FROM dd
  UNION ALL SELECT grp, 5, 'page (npm ci + vite build)', page FROM dd
  UNION ALL SELECT grp, 6, 'test (compile + run)', test FROM dd
  UNION ALL SELECT grp, 7, 'tpage (page tests)', tpage FROM dd
  UNION ALL SELECT grp, 8, 'other (upload, post steps)', other FROM dd
  UNION ALL SELECT grp, 9, 'tjob (whole test job)', tjob FROM dd
  UNION ALL SELECT grp, 10, 'lint_wait (lint ended after test)', lint_wait FROM dd
  UNION ALL SELECT grp, 11, 'summary (job fast)', summary FROM dd
  UNION ALL SELECT grp, 12, 'post (engine, after the run ended)', post FROM dd
  UNION ALL SELECT grp, 13, 'gh_run (GitHub run)', gh_run FROM dd
  UNION ALL SELECT grp, 14, 'total (engine tool run)', total FROM dd;
.mode list
.print '== 1. segments by group: grp | metric | n | mean | median | min | max'
WITH r AS (SELECT grp, o, metric, x, row_number() OVER (PARTITION BY grp, metric ORDER BY x) AS rn,
                  count(*) OVER (PARTITION BY grp, metric) AS n FROM m)
SELECT grp, metric, n, round(avg(x), 1), avg(CASE WHEN rn IN ((n + 1) / 2, (n + 2) / 2) THEN x END), min(x), max(x)
FROM r GROUP BY grp, metric ORDER BY grp, o;

.print '== 2. share of the mean call: ci, all (segment mean / total mean, percent)'
SELECT 'pre', round(100.0 * avg(pre) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'qtest', round(100.0 * avg(qtest) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'setup', round(100.0 * avg(setup) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'cache', round(100.0 * avg(cache) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'page', round(100.0 * avg(page) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'test', round(100.0 * avg(test) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'tpage', round(100.0 * avg(tpage) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'other', round(100.0 * avg(other) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'lint_wait', round(100.0 * avg(lint_wait) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'summary', round(100.0 * avg(summary) / avg(total), 1) FROM d WHERE tool = 'ci'
UNION ALL SELECT 'post', round(100.0 * avg(post) / avg(total), 1) FROM d WHERE tool = 'ci';

.print '== 3. check: the largest gap between the segments and the tool run, and the rows it concerns'
SELECT max(abs(residual)), count(*), sum(residual <> 0) FROM d;

.print '== 4. the lint job on the critical path: tool | runs | lint ended after test | mean wait of those'
SELECT tool, count(*), sum(lint_wait > 0), round(avg(CASE WHEN lint_wait > 0 THEN lint_wait END), 1)
FROM d GROUP BY tool;
SELECT kind, count(*), sum(lint_wait > 0), round(avg(CASE WHEN lint_wait > 0 THEN lint_wait END), 1)
FROM d WHERE tool = 'ci' GROUP BY kind;

.print '== 5. the restore of main build state, ci only: hit | n | mean cache | mean test | mean cache+test | mean tjob'
SELECT hit, count(*), round(avg(cache), 1), round(avg(test), 1), round(avg(cache + test), 1), round(avg(tjob), 1)
FROM d WHERE tool = 'ci' GROUP BY hit;
WITH r AS (SELECT hit, cache + test AS x, row_number() OVER (PARTITION BY hit ORDER BY cache + test) AS rn,
                  count(*) OVER (PARTITION BY hit) AS n FROM d WHERE tool = 'ci')
SELECT hit, n, avg(CASE WHEN rn IN ((n + 1) / 2, (n + 2) / 2) THEN x END) AS median_cache_plus_test FROM r GROUP BY hit;

.print '== 5b. the same by day (2026-11-04 starts at epoch 1793750400): day | hit | n | mean cache | mean test | mean sum'
SELECT CASE WHEN created < 1793750400 THEN '11-03' ELSE '11-04' END AS day, hit, count(*), round(avg(cache), 1),
  round(avg(test), 1), round(avg(cache + test), 1)
FROM d WHERE tool = 'ci' GROUP BY day, hit ORDER BY day, hit;

.print '== 5c. the restore alone, ci_before: n | mean | min | max'
SELECT count(*), round(avg(cache), 1), min(cache), max(cache) FROM d WHERE tool = 'ci_before';

.print '== 6. waiting for a runner, test job and lint job that ran: grp | n | mean | median'
WITH r AS (SELECT grp, qtest AS x, row_number() OVER (PARTITION BY grp ORDER BY qtest) AS rn,
                  count(*) OVER (PARTITION BY grp) AS n FROM dd)
SELECT 'test job: ' || grp, n, round(avg(x), 1), avg(CASE WHEN rn IN ((n + 1) / 2, (n + 2) / 2) THEN x END) FROM r GROUP BY grp;
WITH r AS (SELECT grp, qlint AS x, row_number() OVER (PARTITION BY grp ORDER BY qlint) AS rn,
                  count(*) OVER (PARTITION BY grp) AS n FROM dd WHERE qlint IS NOT NULL)
SELECT 'lint job: ' || grp, n, round(avg(x), 1), avg(CASE WHEN rn IN ((n + 1) / 2, (n + 2) / 2) THEN x END) FROM r GROUP BY grp;

.print '== 7. median and its 95% rank interval (ranks j and k below), GitHub run and tool run: grp | metric | n | j | k | median | lo | hi'
.print '   j = floor((n - 1.96 sqrt n) / 2), k = ceil(1 + (n + 1.96 sqrt n) / 2), at least 1 and at most n'
WITH r AS (SELECT grp, metric, x, row_number() OVER (PARTITION BY grp, metric ORDER BY x) AS rn,
                  count(*) OVER (PARTITION BY grp, metric) AS n FROM m WHERE o IN (13, 14)),
b AS (SELECT grp, metric, n, max(1, cast((n - 1.96 * sqrt(n)) / 2 AS INTEGER)) AS j,
             min(n, cast(ceil(1 + (n + 1.96 * sqrt(n)) / 2) AS INTEGER)) AS k FROM r GROUP BY grp, metric)
SELECT b.grp, b.metric, b.n, b.j, b.k,
  (SELECT avg(x) FROM r WHERE r.grp = b.grp AND r.metric = b.metric AND rn IN ((b.n + 1) / 2, (b.n + 2) / 2)),
  (SELECT x FROM r WHERE r.grp = b.grp AND r.metric = b.metric AND rn = b.j),
  (SELECT x FROM r WHERE r.grp = b.grp AND r.metric = b.metric AND rn = b.k)
FROM b ORDER BY b.metric, b.grp;
