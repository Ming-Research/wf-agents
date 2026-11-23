-- The queries on the engine's records behind research/tree-reading-measured.md, each asked through `query` on
-- 2026-11-22. Works end and begin, so a later run gives more rows; the report gives the numbers as they were that day.

-- Q1. The first round of each run of the cells that read the design tree: model, calls, context and cost.
SELECT s.work, s.cell, s.run, r.model, m.calls, m.first_context, m.added, sp.usd_micros, sp.context_tokens
FROM started s
JOIN measured m ON m.work = s.work AND m.run = s.run AND m.round = 1 AND m.attempt = 1
JOIN spent sp ON sp.work = s.work AND sp.run = s.run AND sp.round = 1
LEFT JOIN ran r ON r.work = s.work AND r.run = s.run AND r.round = 1 AND r.attempt = 1
WHERE s.work BETWEEN 11 AND 30 AND s.cell IN ('R0', 'P1', 'P2', 'D1', 'T1', 'T2', 'T3')
ORDER BY s.work, s.run;

-- Q2. What each cell's runs cost in each work.
SELECT s.work, s.cell, COUNT(DISTINCT s.run) AS runs, ROUND(SUM(sp.usd_micros) / 1e6, 2) AS usd
FROM started s JOIN spent sp ON sp.work = s.work AND sp.run = s.run
WHERE s.work BETWEEN 11 AND 30
GROUP BY s.work, s.cell ORDER BY s.work, s.cell;

-- Q3. The new sessions of each line, and why each began.
SELECT work, line, COUNT(*) AS n, group_concat(substr(why, 1, 60), ' | ') AS whys
FROM sessions WHERE work BETWEEN 11 AND 30 GROUP BY work, line ORDER BY work, line;

-- Q4. Each version of the design tree a work had, with the time it was written and its git tree's id.
SELECT c.work, c.version, (SELECT MAX(mo.time) FROM moments mo WHERE mo.work = c.work AND mo.place <= c.version) AS t,
       substr(c.content, 42, 10) AS tree_id
FROM contents c WHERE c.block = 'tree' AND c.work >= 11 ORDER BY t;

-- Q5. When each run of R0 and D1 began.
SELECT s.work, s.run, s.cell, (SELECT MAX(mo.time) FROM moments mo WHERE mo.work = s.work AND mo.place <= s.at) AS t
FROM started s WHERE s.cell IN ('R0', 'D1') AND s.work >= 11 ORDER BY t;

-- Q6. The points of the two reviews against the design tree, of the plan and of the change.
SELECT c.work, c.block, c.version, c.content
FROM contents c WHERE c.block IN ('tree_review', 'design_review') AND c.work >= 11 ORDER BY c.work, c.block, c.version;

-- Q7. Where the work of #42, which also changes docs/data-and-context.md, stands.
SELECT s.run, s.cell, (SELECT COUNT(*) FROM finished f WHERE f.work = 42 AND f.run = s.run) AS finished
FROM started s WHERE s.work = 42 ORDER BY s.run;
