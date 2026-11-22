-- Queries behind research/i1-round-cost-measured.md, as run with the engine's `query` tool on 2026-11-22.
--
-- The store keeps growing, so every query takes only the rounds that began at or before 2026-11-22 11:45:00 UTC
-- (1795347900000 ms). A round's time is that of the latest moment (`moments`) at or before the entry in `rounds` that
-- began it; the entries of a run's `rounds` in order are its rounds 1, 2, ..., which are `spent`'s `round`. Run later,
-- the queries give the same numbers only while the cutoff stays in; without it they give more.
-- The tool counts, error counts, timings and compaction records in the report come from the round streams in the data
-- directory (`rounds/<work>/<run>-<round>.stream.jsonl`), which the store does not hold: see i1-round-stream.py.

-- Q1: every round of the implementation cell that has a measured record: cost, tokens, calls, context at the start and
-- the end. (202 rounds, $233.56. The cell's other 60 rounds, $25.60, have no measured record; Q3 counts all 262.)
with rt as (
  select r.work, r.run, row_number() over (partition by r.work, r.run order by r.at) as round,
         (select mo.time from moments mo where mo.work = r.work and mo.place <= r.at order by mo.place desc limit 1) as t
  from rounds r)
select sp.work, sp.run, sp.round, sp.usd_micros / 1e6 as usd, sp.output_tokens, sp.cached_tokens, m.cache_written_tokens,
       sp.context_tokens as ctx_end, sp.compactions, m.calls, m.turns, m.first_context, m.added, m.thinking_tokens
from started s
join spent sp on sp.work = s.work and sp.run = s.run
join rt on rt.work = sp.work and rt.run = sp.run and rt.round = sp.round
join measured m on m.work = sp.work and m.run = sp.run and m.round = sp.round and m.attempt = 1
where s.cell = 'I1' and rt.t <= 1795347900000
order by sp.work, sp.run, sp.round;

-- Q2: what each cell's rounds came to. (All cells together: 850 rounds, $443.41; the cell I1: 262 rounds, $259.16, 58%.)
with rt as (
  select r.work, r.run, row_number() over (partition by r.work, r.run order by r.at) as round,
         (select mo.time from moments mo where mo.work = r.work and mo.place <= r.at order by mo.place desc limit 1) as t
  from rounds r)
select s.cell, count(*) as n, round(sum(sp.usd_micros) / 1e6, 2) as usd, round(avg(sp.usd_micros) / 1e6, 3) as avg_usd,
       round(sum(sp.output_tokens) / 1e3) as out_k, round(sum(sp.cached_tokens) / 1e6, 1) as cached_m, sum(sp.compactions) as compactions
from started s
join spent sp on sp.work = s.work and sp.run = s.run
join rt on rt.work = sp.work and rt.run = sp.run and rt.round = sp.round
where rt.t <= 1795347900000
group by s.cell order by sum(sp.usd_micros) desc;

-- Q3: the model of each round of the cell, with what it cost (the first try of the round). 31 rounds, $15.26, have no
-- `ran` record, so the model is `none`.
with rt as (
  select r.work, r.run, row_number() over (partition by r.work, r.run order by r.at) as round,
         (select mo.time from moments mo where mo.work = r.work and mo.place <= r.at order by mo.place desc limit 1) as t
  from rounds r)
select coalesce(x.model, 'none') as model, count(*) as rounds, round(sum(sp.usd_micros) / 1e6, 2) as usd
from started s
join spent sp on sp.work = s.work and sp.run = s.run
join rt on rt.work = sp.work and rt.run = sp.run and rt.round = sp.round
left join ran x on x.work = sp.work and x.run = sp.run and x.round = sp.round and x.attempt = 1
where s.cell = 'I1' and rt.t <= 1795347900000
group by x.model;

-- Q4: the rows the per-token prices were fitted on: rounds of every cell, with a measured and a ran record, whose cost the
-- program reported and that were not compacted. For each model the prices are the least-squares solution of
-- usd_micros = read * cache_read_tokens + write * cache_written_tokens + out * output_tokens, each row weighted by
-- 1 / usd_micros^2. Result: Sonnet 5.5 (300 rounds) $0.100, $4.006, $10.017 per million tokens, median error 0.13%;
-- Opus 5.5 (246 rounds) $0.199, $8.023, $20.809, median error 0.65%.
with rt as (
  select r.work, r.run, row_number() over (partition by r.work, r.run order by r.at) as round,
         (select mo.time from moments mo where mo.work = r.work and mo.place <= r.at order by mo.place desc limit 1) as t
  from rounds r)
select x.model, sp.usd_micros, m.cache_read_tokens, m.cache_written_tokens, sp.output_tokens
from spent sp
join rt on rt.work = sp.work and rt.run = sp.run and rt.round = sp.round
join measured m on m.work = sp.work and m.run = sp.run and m.round = sp.round and m.attempt = 1
join ran x on x.work = sp.work and x.run = sp.run and x.round = sp.round and x.attempt = 1
where rt.t <= 1795347900000 and m.priced = 0 and sp.usd_micros > 0 and sp.compactions = 0;

-- Q5: the rounds of the cell by how many calls they made, among those with a measured record.
-- Result: 1 to 2 calls 98 rounds $10.56; 3 to 39 calls 90 rounds $73.61; 40 or more 14 rounds $149.39; together 202 rounds
-- $233.56. The cost shares by kind of token (reads 55.7%, writes 22.6%, output 16.7% of $233.56) are Q4's prices
-- applied to Q1's tokens; the remaining 5% is what the three counts do not price.
with rt as (
  select r.work, r.run, row_number() over (partition by r.work, r.run order by r.at) as round,
         (select mo.time from moments mo where mo.work = r.work and mo.place <= r.at order by mo.place desc limit 1) as t
  from rounds r)
select case when m.calls <= 2 then 'a: 1-2 calls' when m.calls < 40 then 'b: 3-39 calls' else 'c: 40 or more calls' end as bucket,
       count(*) as rounds, round(sum(sp.usd_micros) / 1e6, 2) as usd
from started s
join spent sp on sp.work = s.work and sp.run = s.run
join rt on rt.work = sp.work and rt.run = sp.run and rt.round = sp.round
join measured m on m.work = sp.work and m.run = sp.run and m.round = sp.round and m.attempt = 1
where s.cell = 'I1' and rt.t <= 1795347900000
group by 1 order by 1;
