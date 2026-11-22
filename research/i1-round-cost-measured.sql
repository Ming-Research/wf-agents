-- Queries behind research/i1-round-cost-measured.md, as run with the engine's `query` tool on 2026-11-22.
-- The tool counts, error counts, timings and compaction records in the report come from the round streams in the
-- data directory (`rounds/<work>/<run>-<round>.stream.jsonl`), which the engine's store does not hold.

-- Q1: every round of the implementation cell: cost, tokens, calls, context at the start and the end.
select s.work, s.run, sp.round, sp.usd_micros / 1e6 as usd, sp.output_tokens, sp.cached_tokens, sp.context_tokens as ctx_end,
       sp.compactions, m.calls, m.turns, m.first_context, m.added, m.thinking_tokens
from started s
join spent sp on sp.work = s.work and sp.run = s.run
join measured m on m.work = s.work and m.run = s.run and m.round = sp.round and m.attempt = 1
where s.cell = 'I1'
order by s.work, s.run, sp.round;

-- Q2: what each cell's rounds came to.
select s.cell, count(*) as n, round(sum(sp.usd_micros) / 1e6, 2) as usd, round(avg(sp.usd_micros) / 1e6, 3) as avg_usd,
       round(sum(sp.output_tokens) / 1e3) as out_k, round(sum(sp.cached_tokens) / 1e6, 1) as cached_m, sum(sp.compactions) as compactions
from started s join spent sp on sp.work = s.work and sp.run = s.run
group by s.cell order by sum(sp.usd_micros) desc;

-- Q3: the model of each round of the cell, with what it cost (first try of the round).
select r.model, count(*) as rounds, round(sum(sp.usd_micros) / 1e6, 1) as usd
from started s
join ran r on r.work = s.work and r.run = s.run and r.attempt = 1
join spent sp on sp.work = s.work and sp.run = s.run and sp.round = r.round
where s.cell = 'I1' group by r.model;

-- Q4: the rows the per-token prices were fitted on (rounds whose cost the program reported, none compacted).
-- The prices are the least-squares solution of usd_micros = read * cache_read + write * cache_written + out * output,
-- each row weighted by 1 / usd_micros^2, taken for one model at a time.
select sp.usd_micros, m.cache_read_tokens, m.cache_written_tokens, sp.output_tokens
from spent sp
join measured m on m.work = sp.work and m.run = sp.run and m.round = sp.round and m.attempt = 1
join ran r on r.work = sp.work and r.run = sp.run and r.round = sp.round and r.attempt = 1
where r.model = :model and m.priced = 0 and sp.usd_micros > 0 and sp.compactions = 0;
