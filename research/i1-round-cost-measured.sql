-- Queries behind research/i1-round-cost-measured.md, as run with the engine's `query` tool on 2026-11-22.
--
-- The store keeps growing, so every query takes only the rounds of runs that began at or before 2026-11-22 11:45:00 UTC
-- (1795347900000 ms): a run's time is that of the latest moment at or before the entry that started it (`moments`).
-- Run later, the queries give the same numbers only if the cutoff stays in; without it they give more.
-- The tool counts, error counts, timings and compaction records in the report come from the round streams in the data
-- directory (`rounds/<work>/<run>-<round>.stream.jsonl`), which the store does not hold: see i1-round-stream.py.

-- Q1: every round of the implementation cell that has a measured record: cost, tokens, calls, context at the start and
-- the end. (202 rounds, $233.56. The cell's other 60 rounds, $25.60, have no measured record; Q3 counts all 262.)
with runs as (
  select s.work, s.run, s.cell,
         (select mo.time from moments mo where mo.work = s.work and mo.place <= s.at order by mo.place desc limit 1) as t
  from started s)
select runs.work, runs.run, sp.round, sp.usd_micros / 1e6 as usd, sp.output_tokens, sp.cached_tokens, m.cache_written_tokens,
       sp.context_tokens as ctx_end, sp.compactions, m.calls, m.turns, m.first_context, m.added, m.thinking_tokens
from runs
join spent sp on sp.work = runs.work and sp.run = runs.run
join measured m on m.work = sp.work and m.run = sp.run and m.round = sp.round and m.attempt = 1
where runs.cell = 'I1' and runs.t <= 1795347900000
order by runs.work, runs.run, sp.round;

-- Q2: what each cell's rounds came to. (All cells together: 850 rounds, $443.42; the cell I1: 262 rounds, $259.16, 58%.)
with runs as (
  select s.work, s.run, s.cell,
         (select mo.time from moments mo where mo.work = s.work and mo.place <= s.at order by mo.place desc limit 1) as t
  from started s)
select runs.cell, count(*) as n, round(sum(sp.usd_micros) / 1e6, 2) as usd, round(avg(sp.usd_micros) / 1e6, 3) as avg_usd,
       round(sum(sp.output_tokens) / 1e3) as out_k, round(sum(sp.cached_tokens) / 1e6, 1) as cached_m, sum(sp.compactions) as compactions
from runs join spent sp on sp.work = runs.work and sp.run = runs.run
where runs.t <= 1795347900000
group by runs.cell order by sum(sp.usd_micros) desc;

-- Q3: the model of each round of the cell, with what it cost (the first try of the round). 31 rounds, $15.26, have no
-- `ran` record, so the model is `none`.
with runs as (
  select s.work, s.run, s.cell,
         (select mo.time from moments mo where mo.work = s.work and mo.place <= s.at order by mo.place desc limit 1) as t
  from started s)
select coalesce(r.model, 'none') as model, count(*) as rounds, round(sum(sp.usd_micros) / 1e6, 2) as usd
from runs
join spent sp on sp.work = runs.work and sp.run = runs.run
left join ran r on r.work = sp.work and r.run = sp.run and r.round = sp.round and r.attempt = 1
where runs.cell = 'I1' and runs.t <= 1795347900000
group by r.model;

-- Q4: the rows the per-token prices were fitted on: rounds of every cell, with a measured and a ran record, whose cost the
-- program reported and that were not compacted. For each model the prices are the least-squares solution of
-- usd_micros = read * cache_read_tokens + write * cache_written_tokens + out * output_tokens, each row weighted by
-- 1 / usd_micros^2. Result: Sonnet 5.5 (300 rounds) $0.100, $4.006, $10.017 per million tokens, median error 0.13%;
-- Opus 5.5 (246 rounds) $0.199, $8.023, $20.809, median error 0.65%.
with runs as (
  select s.work, s.run,
         (select mo.time from moments mo where mo.work = s.work and mo.place <= s.at order by mo.place desc limit 1) as t
  from started s)
select r.model, sp.usd_micros, m.cache_read_tokens, m.cache_written_tokens, sp.output_tokens
from runs
join spent sp on sp.work = runs.work and sp.run = runs.run
join measured m on m.work = sp.work and m.run = sp.run and m.round = sp.round and m.attempt = 1
join ran r on r.work = sp.work and r.run = sp.run and r.round = sp.round and r.attempt = 1
where runs.t <= 1795347900000 and m.priced = 0 and sp.usd_micros > 0 and sp.compactions = 0;

-- Q5: the rounds of the cell by how many calls they made, with the models of the rounds that have a measured record.
-- Result: 1 to 2 calls 98 rounds $10.56; 3 to 39 calls 90 rounds $73.61; 40 or more 14 rounds $149.39; together 202 rounds
-- $233.56. The cost shares by kind of token (reads 55.7%, writes 22.6%, output 16.7% of $233.56) are Q4's prices
-- applied to Q1's tokens; the remaining 5% is what the three counts do not price.
with runs as (
  select s.work, s.run, s.cell,
         (select mo.time from moments mo where mo.work = s.work and mo.place <= s.at order by mo.place desc limit 1) as t
  from started s)
select case when m.calls <= 2 then 'a: 1-2 calls' when m.calls < 40 then 'b: 3-39 calls' else 'c: 40 or more calls' end as bucket,
       count(*) as rounds, round(sum(sp.usd_micros) / 1e6, 2) as usd
from runs
join spent sp on sp.work = runs.work and sp.run = runs.run
join measured m on m.work = sp.work and m.run = sp.run and m.round = sp.round and m.attempt = 1
where runs.cell = 'I1' and runs.t <= 1795347900000
group by 1 order by 1;
