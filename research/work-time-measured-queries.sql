-- The queries behind research/work-time-measured.md that read the engine's database (tables rounds, tool_runs, works,
-- slot_waits, work_routes). They are run with the engine's `query` tool, one statement at a time (the database is not in
-- the repository), and each is the text that was run on 2026-11-05 (UTC); the report names them Q1 to Q9. Whom it serves:
-- a reader who wants to re-run a number of the report; it is removed with the report.
--
-- Times in rounds and tool_runs are UTC milliseconds. The moments used:
--   1793703148000  2026-11-03 10:52:28 UTC, the first `ci` that ran named tests (the fast CI, item #244; the start of
--                  every set below: a work counts when its first round started at or after it)
--   1793836686000  2026-11-04 23:58:06 UTC, when the first job of a run on a 14900k runner was created (run 41937774408)
--   1793838000000  2026-11-05 00:20:00 UTC, the start of the group "after": a merged work is "after" when its first `ci`
--                  of 10 s or more started at or after it. Every `ci` and `ci_before` run of the works that are "after"
--                  was read from GitHub and ran on a runner labelled 14900k (51 of 51, report section 2); the last run
--                  read on a hosted runner (ubuntu-24.04) was created at 2026-11-05 00:03:45 UTC (run 41938479809)
--   1793885427000  2026-11-05 13:30:27 UTC, the cutoff that freezes the sets (the newest round of the sample started at
--                  13:30:26): queries Q2 to Q8 count only what ended before it, so that a re-run gives the same sets.
--                  Q1 and Q9 have no cutoff and give the rows they gave, and more if re-run.
-- Groups, in Q2 to Q7: `before` merged before 2026-11-04 23:58:06 UTC; `after` first `ci` at or after 00:20:00 UTC on
-- 2026-11-05; `between` the rest (works that ran some of their CI on each kind of runner).
-- A work is a small fix (no deep research) when it had no round at cell D1; `work_routes` agrees but for work 287,
-- which has no route recorded and no D1 round.
-- A round's class: `agent` has a model; `wait_ask` is cell A2 (to the architect) or A3 (to the owner); `merge` has a
-- `merge` tool call; `ci_tools` has `ci`, `ci_before`, `full_ci`, `measure` or `watch_main`; `light_tools` has only
-- `query` or `gh`; `engine` is the rest. A gap is the next round's start less the previous round's end; its part that
-- overlaps a row of slot_waits is `gap_slot`, the rest `gap_other`. A work's span is the first round's start to the end
-- of the round that merged (the round of its `merge` call whose result is a 40-character commit id).

-- Q1. The merged works since the fast CI: when each started, minutes to the merge and to the last round (the figure
-- that item #305 gives for works 302 and 316), the rounds, the D1 rounds, the route and the round of the merge.
WITH f AS (SELECT work, min(started_at) AS first_at, count(*) AS n_rounds, sum(cell='D1') AS d1, max(coalesce(ended_at,started_at)) AS last_end FROM rounds GROUP BY work),
m AS (SELECT work, max(ended_at) AS merged_at, max(round) AS merge_round FROM tool_runs WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 GROUP BY work)
SELECT w.id AS work, w.item, datetime(f.first_at/1000,'unixepoch') AS first_at, round((m.merged_at-f.first_at)/60000.0,1) AS to_merge_min,
       round((f.last_end-f.first_at)/60000.0,1) AS to_last_min, f.n_rounds, f.d1, m.merge_round,
       (SELECT route FROM work_routes r WHERE r.work=w.id) AS route
FROM works w JOIN f ON f.work=w.id JOIN m ON m.work=w.id
WHERE w.graph='engineer' AND f.first_at >= 1793703148000 ORDER BY m.merged_at;

-- Q2. Each merged work's span split into the classes of its rounds and gaps, in minutes: one row per work. `resid_s` is
-- the span less the sum of the classes in seconds (0: the rounds of a work do not overlap, `overlaps` counts those that do).
WITH f AS (SELECT work, min(started_at) AS first_at FROM rounds GROUP BY work),
m AS (SELECT work, max(round) AS mr, max(ended_at) AS me FROM tool_runs WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND ended_at < 1793885427000 GROUP BY work),
c AS (SELECT w.id AS work, f.first_at, m.mr, m.me, (SELECT min(t.started_at) FROM tool_runs t WHERE t.work=w.id AND t.tool='ci' AND t.ended_at-t.started_at>=10000) AS first_ci, (SELECT count(*) FROM rounds d WHERE d.work=w.id AND d.cell='D1') AS d1 FROM works w JOIN f ON f.work=w.id JOIN m ON m.work=w.id WHERE w.graph='engineer' AND f.first_at >= 1793703148000),
rr AS (SELECT r.work, r.n, r.cell, r.model IS NOT NULL AS agent, r.started_at AS s, coalesce(r.ended_at, r.started_at) AS e, LAG(coalesce(r.ended_at, r.started_at)) OVER (PARTITION BY r.work ORDER BY r.n) AS pe FROM rounds r JOIN c ON c.work=r.work WHERE r.n <= c.mr),
k AS (SELECT rr.*, CASE WHEN agent THEN 'agent' WHEN cell IN ('A2','A3') THEN 'wait_ask'
        WHEN EXISTS (SELECT 1 FROM tool_runs t WHERE t.work=rr.work AND t.round=rr.n AND t.tool='merge') THEN 'merge'
        WHEN EXISTS (SELECT 1 FROM tool_runs t WHERE t.work=rr.work AND t.round=rr.n AND t.tool IN ('ci','ci_before','full_ci','measure','watch_main')) THEN 'ci_tools'
        WHEN EXISTS (SELECT 1 FROM tool_runs t WHERE t.work=rr.work AND t.round=rr.n) THEN 'light_tools' ELSE 'engine' END AS cls,
      rr.e-rr.s AS dur, CASE WHEN rr.pe IS NULL THEN 0 ELSE rr.s-rr.pe END AS gap FROM rr),
g AS (SELECT k.*, min(max(k.gap,0), (SELECT coalesce(sum(max(0, min(coalesce(sw.until, k.s), k.s) - max(sw.since, k.s-k.gap))),0) FROM slot_waits sw WHERE sw.work=k.work)) AS gslot FROM k)
SELECT g.work, CASE WHEN c.first_ci >= 1793838000000 THEN 'after' WHEN c.me < 1793836686000 THEN 'before' ELSE 'between' END AS grp, c.d1,
       datetime(c.first_at/1000,'unixepoch') AS first_at, round((max(g.e)-min(g.s))/60000.0,2) AS span,
       round(coalesce(sum(CASE WHEN cls='agent' THEN dur END),0)/60000.0,2) AS agent,
       round(coalesce(sum(CASE WHEN cls='ci_tools' THEN dur END),0)/60000.0,2) AS ci_tools,
       round(coalesce(sum(CASE WHEN cls='merge' THEN dur END),0)/60000.0,2) AS merge,
       round(coalesce(sum(CASE WHEN cls='light_tools' THEN dur END),0)/60000.0,2) AS light,
       round(coalesce(sum(CASE WHEN cls='wait_ask' THEN dur END),0)/60000.0,2) AS wait_ask,
       round(coalesce(sum(CASE WHEN cls='engine' THEN dur END),0)/60000.0,2) AS engine,
       round(sum(gslot)/60000.0,2) AS gap_slot, round((sum(gap)-sum(gslot))/60000.0,2) AS gap_other,
       round((max(g.e)-min(g.s)-sum(dur)-sum(gap))/1000.0,1) AS resid_s, sum(gap<0) AS overlaps, count(*) AS rounds
FROM g JOIN c USING(work) GROUP BY g.work ORDER BY grp, g.work;

-- Q3. The groups of Q2 summed up: for each group, all its works and those without a D1 round, the number of works, the
-- median and the sum of each class in minutes, and the class's share of the sum of the spans.
WITH f AS (SELECT work, min(started_at) AS first_at FROM rounds GROUP BY work),
m AS (SELECT work, max(round) AS mr, max(ended_at) AS me FROM tool_runs WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND ended_at < 1793885427000 GROUP BY work),
c0 AS (SELECT w.id AS work, f.first_at, m.mr, m.me, (SELECT min(t.started_at) FROM tool_runs t WHERE t.work=w.id AND t.tool='ci' AND t.ended_at-t.started_at>=10000) AS first_ci, (SELECT count(*) FROM rounds d WHERE d.work=w.id AND d.cell='D1') AS d1 FROM works w JOIN f ON f.work=w.id JOIN m ON m.work=w.id WHERE w.graph='engineer' AND f.first_at >= 1793703148000),
c AS (SELECT c0.*, CASE WHEN first_ci >= 1793838000000 THEN 'after' WHEN me < 1793836686000 THEN 'before' ELSE 'between' END AS grp FROM c0),
rr AS (SELECT r.work, r.n, r.cell, r.model IS NOT NULL AS agent, r.started_at AS s, coalesce(r.ended_at, r.started_at) AS e, LAG(coalesce(r.ended_at, r.started_at)) OVER (PARTITION BY r.work ORDER BY r.n) AS pe FROM rounds r JOIN c ON c.work=r.work WHERE r.n <= c.mr),
k AS (SELECT rr.*, CASE WHEN agent THEN 'agent' WHEN cell IN ('A2','A3') THEN 'wait_ask'
        WHEN EXISTS (SELECT 1 FROM tool_runs t WHERE t.work=rr.work AND t.round=rr.n AND t.tool='merge') THEN 'merge'
        WHEN EXISTS (SELECT 1 FROM tool_runs t WHERE t.work=rr.work AND t.round=rr.n AND t.tool IN ('ci','ci_before','full_ci','measure','watch_main')) THEN 'ci_tools'
        WHEN EXISTS (SELECT 1 FROM tool_runs t WHERE t.work=rr.work AND t.round=rr.n) THEN 'light_tools' ELSE 'engine' END AS cls,
      rr.e-rr.s AS dur, CASE WHEN rr.pe IS NULL THEN 0 ELSE rr.s-rr.pe END AS gap FROM rr),
g AS (SELECT k.*, min(max(k.gap,0), (SELECT coalesce(sum(max(0, min(coalesce(sw.until, k.s), k.s) - max(sw.since, k.s-k.gap))),0) FROM slot_waits sw WHERE sw.work=k.work)) AS gslot FROM k),
pw AS (SELECT g.work, c.grp, (c.d1=0) AS small, max(g.e)-min(g.s) AS span,
         sum(CASE WHEN cls='agent' THEN dur ELSE 0 END) AS agent, sum(CASE WHEN cls='ci_tools' THEN dur ELSE 0 END) AS ci_tools,
         sum(CASE WHEN cls='merge' THEN dur ELSE 0 END) AS merge, sum(CASE WHEN cls='light_tools' THEN dur ELSE 0 END) AS light_tools,
         sum(CASE WHEN cls='wait_ask' THEN dur ELSE 0 END) AS wait_ask, sum(CASE WHEN cls='engine' THEN dur ELSE 0 END) AS engine,
         sum(gslot) AS gap_slot, sum(gap)-sum(gslot) AS gap_other
       FROM g JOIN c USING(work) GROUP BY g.work),
vals AS (SELECT grp, small, 'span' AS m, span AS x FROM pw UNION ALL SELECT grp, small, 'agent', agent FROM pw
         UNION ALL SELECT grp, small, 'ci_tools', ci_tools FROM pw UNION ALL SELECT grp, small, 'merge', merge FROM pw
         UNION ALL SELECT grp, small, 'light_tools', light_tools FROM pw UNION ALL SELECT grp, small, 'wait_ask', wait_ask FROM pw
         UNION ALL SELECT grp, small, 'engine', engine FROM pw UNION ALL SELECT grp, small, 'gap_slot', gap_slot FROM pw
         UNION ALL SELECT grp, small, 'gap_other', gap_other FROM pw),
sv AS (SELECT grp || ' / all' AS scope, m, x FROM vals UNION ALL SELECT grp || ' / no D1 round', m, x FROM vals WHERE small=1),
r AS (SELECT scope, m, x, row_number() OVER (PARTITION BY scope, m ORDER BY x) AS rn, count(*) OVER (PARTITION BY scope, m) AS n FROM sv),
a AS (SELECT scope, m, count(*) AS n, avg(CASE WHEN rn IN ((n+1)/2,(n+2)/2) THEN x END) AS med_ms, sum(x) AS sum_ms FROM r GROUP BY scope, m)
SELECT scope, m, n, round(med_ms/60000.0,2) AS median_min, round(sum_ms/60000.0,1) AS sum_min,
       round(100.0*sum_ms/(SELECT a2.sum_ms FROM a a2 WHERE a2.scope=a.scope AND a2.m='span'),1) AS share_pct
FROM a ORDER BY scope, CASE m WHEN 'span' THEN 0 ELSE 1 END, m;

-- Q4. The gaps of 30 s or more of Q2's works, summed by the pair of cells they lie between (previous round's cell, next
-- round's cell): how many, their minutes, the longest, and the minutes of them that overlap a wait for a slot.
WITH f AS (SELECT work, min(started_at) AS first_at FROM rounds GROUP BY work),
m AS (SELECT work, max(round) AS mr, max(ended_at) AS me FROM tool_runs WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND ended_at < 1793885427000 GROUP BY work),
c0 AS (SELECT w.id AS work, f.first_at, m.mr, m.me, (SELECT min(t.started_at) FROM tool_runs t WHERE t.work=w.id AND t.tool='ci' AND t.ended_at-t.started_at>=10000) AS first_ci, (SELECT count(*) FROM rounds d WHERE d.work=w.id AND d.cell='D1') AS d1 FROM works w JOIN f ON f.work=w.id JOIN m ON m.work=w.id WHERE w.graph='engineer' AND f.first_at >= 1793703148000),
c AS (SELECT c0.*, CASE WHEN first_ci >= 1793838000000 THEN 'after' WHEN me < 1793836686000 THEN 'before' ELSE 'between' END AS grp FROM c0),
rr AS (SELECT r.work, r.n, r.cell, r.started_at AS s, coalesce(r.ended_at, r.started_at) AS e, LAG(coalesce(r.ended_at, r.started_at)) OVER (PARTITION BY r.work ORDER BY r.n) AS pe, LAG(r.cell) OVER (PARTITION BY r.work ORDER BY r.n) AS pcell FROM rounds r JOIN c ON c.work=r.work WHERE r.n <= c.mr),
k AS (SELECT rr.*, CASE WHEN rr.pe IS NULL THEN 0 ELSE rr.s-rr.pe END AS gap FROM rr),
g AS (SELECT k.*, min(max(k.gap,0), (SELECT coalesce(sum(max(0, min(coalesce(sw.until, k.s), k.s) - max(sw.since, k.s-k.gap))),0) FROM slot_waits sw WHERE sw.work=k.work)) AS gslot FROM k)
SELECT c.grp, g.pcell, g.cell, count(*) AS n, round(sum(g.gap)/60000.0,1) AS gap_min, round(max(g.gap)/60000.0,1) AS max_min, round(sum(g.gslot)/60000.0,1) AS slot_min
FROM g JOIN c USING(work) WHERE g.gap > 30000 GROUP BY c.grp, g.pcell, g.cell ORDER BY c.grp, gap_min DESC LIMIT 45;

-- Q5. The gaps of more than 120 s one by one, longest first: the work, the round number, the two cells, the gap and the
-- part of it that overlaps a wait for a slot in seconds, and when the next round started. Several works resuming within
-- seconds of one another (2026-11-04 21:18:52 to :54, 22:29:56 to 22:30:03, 2026-11-05 00:03:09 to :10 and 00:19:35 to :36)
-- is what Q6 calls an engine stop.
WITH f AS (SELECT work, min(started_at) AS first_at FROM rounds GROUP BY work),
m AS (SELECT work, max(round) AS mr, max(ended_at) AS me FROM tool_runs WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND ended_at < 1793885427000 GROUP BY work),
c0 AS (SELECT w.id AS work, f.first_at, m.mr, m.me, (SELECT min(t.started_at) FROM tool_runs t WHERE t.work=w.id AND t.tool='ci' AND t.ended_at-t.started_at>=10000) AS first_ci FROM works w JOIN f ON f.work=w.id JOIN m ON m.work=w.id WHERE w.graph='engineer' AND f.first_at >= 1793703148000),
c AS (SELECT c0.*, CASE WHEN first_ci >= 1793838000000 THEN 'after' WHEN me < 1793836686000 THEN 'before' ELSE 'between' END AS grp FROM c0),
rr AS (SELECT r.work, r.n, r.cell, r.started_at AS s, coalesce(r.ended_at, r.started_at) AS e, LAG(coalesce(r.ended_at, r.started_at)) OVER (PARTITION BY r.work ORDER BY r.n) AS pe, LAG(r.cell) OVER (PARTITION BY r.work ORDER BY r.n) AS pcell FROM rounds r JOIN c ON c.work=r.work WHERE r.n <= c.mr),
k AS (SELECT rr.*, CASE WHEN rr.pe IS NULL THEN 0 ELSE rr.s-rr.pe END AS gap FROM rr),
g AS (SELECT k.*, min(max(k.gap,0), (SELECT coalesce(sum(max(0, min(coalesce(sw.until, k.s), k.s) - max(sw.since, k.s-k.gap))),0) FROM slot_waits sw WHERE sw.work=k.work)) AS gslot FROM k)
SELECT g.work, c.grp, g.n, g.pcell, g.cell, round(g.gap/1000.0) AS gap_s, round(g.gslot/1000.0) AS slot_s, datetime(g.s/1000,'unixepoch') AS at_utc
FROM g JOIN c USING(work) WHERE g.gap > 120000 ORDER BY g.gap DESC LIMIT 60;

-- Q6. Every gap classed by what it lies between, for the groups of Q2 (with and without a D1 round): the part of a gap
-- that overlaps a wait for a slot is counted apart (`slot_min`), the rest (`rest_min`) by its class: `D3 to D3` waits
-- for the owner's review of a design, `M1 to M1` at the merge gate (for a change to the flow, the owner's approval),
-- `engine stop` is a gap of 60 s or more that ends within 10 s of another work's gap of 60 s or more (inferred to be
-- the engine not running, as for an installation; the records do not say), `other, 60 s or more` and `under 60 s`.
WITH f AS (SELECT work, min(started_at) AS first_at FROM rounds GROUP BY work),
m AS (SELECT work, max(round) AS mr, max(ended_at) AS me FROM tool_runs WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND ended_at < 1793885427000 GROUP BY work),
c0 AS (SELECT w.id AS work, f.first_at, m.mr, m.me, (SELECT min(t.started_at) FROM tool_runs t WHERE t.work=w.id AND t.tool='ci' AND t.ended_at-t.started_at>=10000) AS first_ci, (SELECT count(*) FROM rounds d WHERE d.work=w.id AND d.cell='D1') AS d1 FROM works w JOIN f ON f.work=w.id JOIN m ON m.work=w.id WHERE w.graph='engineer' AND f.first_at >= 1793703148000),
c AS (SELECT c0.*, CASE WHEN first_ci >= 1793838000000 THEN 'after' WHEN me < 1793836686000 THEN 'before' ELSE 'between' END AS grp FROM c0),
rr AS (SELECT r.work, r.n, r.cell, r.started_at AS s, coalesce(r.ended_at, r.started_at) AS e, LAG(coalesce(r.ended_at, r.started_at)) OVER (PARTITION BY r.work ORDER BY r.n) AS pe, LAG(r.cell) OVER (PARTITION BY r.work ORDER BY r.n) AS pcell FROM rounds r JOIN c ON c.work=r.work WHERE r.n <= c.mr),
k AS (SELECT rr.*, CASE WHEN rr.pe IS NULL THEN 0 ELSE rr.s-rr.pe END AS gap FROM rr),
g AS (SELECT k.*, min(max(k.gap,0), (SELECT coalesce(sum(max(0, min(coalesce(sw.until, k.s), k.s) - max(sw.since, k.s-k.gap))),0) FROM slot_waits sw WHERE sw.work=k.work)) AS gslot FROM k),
gg AS (SELECT g.*, g.gap-g.gslot AS rest,
         CASE WHEN g.gap-g.gslot <= 0 THEN 'none'
              WHEN g.pcell='D3' AND g.cell='D3' THEN 'D3 to D3 (design review)'
              WHEN g.pcell='M1' AND g.cell='M1' THEN 'M1 to M1 (merge gate)'
              WHEN g.gap>=60000 AND EXISTS (SELECT 1 FROM g g2 WHERE g2.work<>g.work AND g2.gap>=60000 AND abs(g2.s-g.s)<=10000) THEN 'engine stop (2 or more works resume within 10 s)'
              WHEN g.gap>=60000 THEN 'other, 60 s or more' ELSE 'under 60 s' END AS cls
       FROM g WHERE g.gap>0)
SELECT c.grp || CASE WHEN c.d1=0 THEN ' / no D1' ELSE ' / D1' END AS scope, gg.cls, count(*) AS n,
       round(sum(gg.rest)/60000.0,1) AS rest_min, round(sum(gg.gslot)/60000.0,1) AS slot_min
FROM gg JOIN c USING(work) GROUP BY 1, gg.cls ORDER BY 1, rest_min DESC;

-- Q7. The cells that took most time, for the works of the group "after" and for all the merged works since the fast
-- CI: rounds, seconds in them (all rounds of the cell, with or without a model), the median round, the model calls
-- (`turns`, the CLI's count; empty for a cell that runs no model) and their mean per round, `est_model_s` and `agent_s`.
-- `est_model_s` is the estimate of the time spent on the model by the fit of Q7b, capped at the round's time, and is
-- REJECTED by Q7c (the report's section 6): do not read it as a measurement. `agent_s` is the seconds of the rounds that
-- ran a model, an upper bound of the time on the model.
WITH f AS (SELECT work, min(started_at) AS first_at FROM rounds GROUP BY work),
m AS (SELECT work, max(round) AS mr, max(ended_at) AS me FROM tool_runs WHERE tool='merge' AND substr(result,1,1)='"' AND length(result)=42 AND ended_at < 1793885427000 GROUP BY work),
c0 AS (SELECT w.id AS work, f.first_at, m.mr, m.me, (SELECT min(t.started_at) FROM tool_runs t WHERE t.work=w.id AND t.tool='ci' AND t.ended_at-t.started_at>=10000) AS first_ci FROM works w JOIN f ON f.work=w.id JOIN m ON m.work=w.id WHERE w.graph='engineer' AND f.first_at >= 1793703148000),
c AS (SELECT c0.*, CASE WHEN first_ci >= 1793838000000 THEN 'after' WHEN me < 1793836686000 THEN 'before' ELSE 'between' END AS grp FROM c0),
rb AS (SELECT r.work, r.n, r.cell, r.model, (coalesce(r.ended_at, r.started_at)-r.started_at)/1000.0 AS y, json_extract(r.usage,'$.turns')*1.0 AS t, json_extract(r.usage,'$.usage.output_tokens')*1.0 AS x, c.grp FROM rounds r JOIN c ON c.work=r.work WHERE r.n <= c.mr),
d AS (SELECT model, json_extract(usage,'$.turns')*1.0 AS t, json_extract(usage,'$.usage.output_tokens')*1.0 AS x, (ended_at-started_at)/1000.0 AS y FROM rounds WHERE model IS NOT NULL AND started_at>=1793703148000 AND ended_at<1793885427000 AND usage IS NOT NULL),
s AS (SELECT model, sum(t*t) AS stt, sum(t*x) AS stx, sum(x*x) AS sxx, sum(t*y) AS sty, sum(x*y) AS sxy FROM d WHERE t<=3 GROUP BY model),
fit AS (SELECT model, (sty*sxx-stx*sxy)/(stt*sxx-stx*stx) AS a, (stt*sxy-stx*sty)/(stt*sxx-stx*stx) AS b FROM s),
pr AS (SELECT rb.work, rb.cell, rb.grp, rb.y, rb.t, rb.x, CASE WHEN rb.model IS NULL THEN NULL ELSE min(rb.y, fit.a*rb.t+fit.b*rb.x) END AS est FROM rb LEFT JOIN fit ON fit.model=rb.model),
sc AS (SELECT 'after' AS scope, pr.* FROM pr WHERE grp='after' UNION ALL SELECT 'all merged since 11-03', pr.* FROM pr),
r AS (SELECT sc.*, row_number() OVER (PARTITION BY scope, cell ORDER BY y) AS rn, count(*) OVER (PARTITION BY scope, cell) AS n FROM sc),
a AS (SELECT scope, cell, count(*) AS rounds, round(sum(y),0) AS sum_s, round(avg(CASE WHEN rn IN ((n+1)/2,(n+2)/2) THEN y END),1) AS median_s, sum(t) AS turns, round(avg(t),1) AS mean_turns, round(sum(est),0) AS est_model_s, round(sum(CASE WHEN t IS NOT NULL THEN y END),0) AS agent_s FROM r GROUP BY scope, cell),
ranked AS (SELECT a.*, row_number() OVER (PARTITION BY scope ORDER BY sum_s DESC) AS rk FROM a)
SELECT * FROM ranked WHERE rk <= 12 ORDER BY scope, rk;

-- Q7b. The fit that Q7 uses, written before it was looked at: for each model, over the rounds since the fast CI that ran a
-- model, called it at most 3 times (so that almost no time of tools is in them) and have a usage record, the seconds of
-- a round fitted by least squares through the origin as a * turns + b * output_tokens. a is in seconds per model call,
-- b in seconds per output token; n the rounds fitted. The rule for using it, written before the run: it holds if, on the
-- rounds of more than 3 calls (not fitted), the fit is above the round's real seconds in at most 10% of them and the median
-- of (real - fitted) is not negative, since those rounds also hold tool time.
WITH d AS (SELECT model, json_extract(usage,'$.turns')*1.0 AS t, json_extract(usage,'$.usage.output_tokens')*1.0 AS x, (ended_at-started_at)/1000.0 AS y FROM rounds WHERE model IS NOT NULL AND started_at>=1793703148000 AND ended_at<1793885427000 AND usage IS NOT NULL),
s AS (SELECT model, count(*) AS n, sum(t*t) AS stt, sum(t*x) AS stx, sum(x*x) AS sxx, sum(t*y) AS sty, sum(x*y) AS sxy, avg(y) AS ay, avg(t) AS at, avg(x) AS ax FROM d WHERE t<=3 GROUP BY model)
SELECT model, n, round(ay,1) AS mean_s, round(at,2) AS mean_turns, round(ax) AS mean_out,
       round((sty*sxx - stx*sxy)/(stt*sxx - stx*stx),3) AS a_s_per_turn, round((stt*sxy - stx*sty)/(stt*sxx - stx*stx),5) AS b_s_per_token
FROM s;

-- Q7c. The rule of Q7b applied: for each model, the fitted rounds and the rounds of more than 3 calls: how many, the
-- share of them where the fit is above the real seconds, the median of (real - fitted) in seconds, and fitted over real.
WITH d AS (SELECT model, json_extract(usage,'$.turns')*1.0 AS t, json_extract(usage,'$.usage.output_tokens')*1.0 AS x, (ended_at-started_at)/1000.0 AS y FROM rounds WHERE model IS NOT NULL AND started_at>=1793703148000 AND ended_at<1793885427000 AND usage IS NOT NULL),
s AS (SELECT model, sum(t*t) AS stt, sum(t*x) AS stx, sum(x*x) AS sxx, sum(t*y) AS sty, sum(x*y) AS sxy FROM d WHERE t<=3 GROUP BY model),
fit AS (SELECT model, (sty*sxx-stx*sxy)/(stt*sxx-stx*stx) AS a, (stt*sxy-stx*sty)/(stt*sxx-stx*stx) AS b FROM s),
v AS (SELECT d.model, d.t, d.y, fit.a*d.t+fit.b*d.x AS pred FROM d JOIN fit ON fit.model=d.model),
r AS (SELECT model, CASE WHEN t<=3 THEN 'fit set turns<=3' ELSE 'turns>3 (not fitted)' END AS bucket, y, pred, y-pred AS res,
             row_number() OVER (PARTITION BY model, (t<=3) ORDER BY y-pred) AS rn, count(*) OVER (PARTITION BY model, (t<=3)) AS n FROM v)
SELECT model, bucket, count(*) AS n, round(100.0*sum(pred>y)/count(*),1) AS pct_pred_gt_actual,
       round(avg(CASE WHEN rn IN ((n+1)/2,(n+2)/2) THEN res END),1) AS median_res_s, round(sum(pred)/sum(y),2) AS pred_share
FROM r GROUP BY model, bucket ORDER BY model, bucket;

-- Q8. The calls of `ci`, `ci_before` and `merge` of the works of the group "after" (317, 328, 330, 331, 332, 334) whose
-- result names a GitHub run, as the rows of the table `m` in research/work-time-measured.sql: tool_runs id, work, round,
-- tool, start and end in epoch seconds, and the run's number cut out of the first check's link. Of the 30 `ci` calls of
-- these works 28 name a run (two did no push), all 16 `ci_before` calls do, and 9 of their 24 `merge` rows do (the
-- others are the row that starts the merge and the row that gives the commit).
SELECT '(' || id || ',' || work || ',' || round || ',''' || tool || ''',' || (started_at/1000) || ',' || (ended_at/1000) || ',' ||
       substr(run, instr(run,'/runs/')+6, instr(run,'/job/')-instr(run,'/runs/')-6) || '),' AS line
FROM (SELECT id, work, round, tool, started_at, ended_at,
             coalesce(json_extract(result,'$.push.checks[0].run'), json_extract(result,'$.ran.checks[0].run'), json_extract(result,'$[0].run')) AS run
      FROM tool_runs WHERE work IN (317,328,330,331,332,334) AND tool IN ('ci','ci_before','merge') AND ended_at IS NOT NULL)
WHERE run IS NOT NULL ORDER BY id;

-- Q9. The `ci` calls of 10 s or more from 2026-11-04 22:14 UTC: when, the seconds, what was asked for and the test job's
-- link, which is how the move of the fast CI to the 14900k runners was seen (calls of 200 to 300 s give way to calls
-- mostly under 150 s between 2026-11-05 00:03 and 00:20 UTC) before the runs' labels were read. No cutoff.
SELECT id, work, round, datetime(started_at/1000,'unixepoch') AS s, (ended_at-started_at)/1000 AS secs,
       json_extract(result,'$.push.verdict') AS v, json_extract(result,'$.push.run') AS kind,
       (SELECT json_extract(value,'$.run') FROM json_each(result,'$.push.checks') WHERE json_extract(value,'$.name')='test') AS test_run
FROM tool_runs WHERE tool='ci' AND started_at >= 1793830000000 AND ended_at-started_at >= 10000 ORDER BY id;

-- The GitHub job times of Q8's runs were read with one command for each run (`gh api repos/<repo>/actions/runs/<run>/jobs`),
-- the jobs sorted by name, in the form of the rows of the table `j` in research/work-time-measured.sql:
--
--   --jq '.jobs | sort_by(.name) | "(" + (.[0].run_id|tostring) + "," + (length|tostring) + ","
--         + (.[-1].labels|map(select(.=="14900k"))|length|tostring) + "," + (.[0].created_at|fromdate|tostring) + ","
--         + ([.[] | ((.started_at|fromdate)-(.created_at|fromdate)), ((.completed_at|fromdate)-(.started_at|fromdate))]
--            | map(tostring) | join(",")) + "),"'
