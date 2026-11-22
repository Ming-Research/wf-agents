# A review that looks again costs less than its first look

What each run of a cell cost, from the engine's store on 2026-11-21, over every work the engine has run. A cell's
first run in a work is counted apart from its later runs. A later run of a review looks only at what changed since it
last looked, in a session whose cache is warm.

| Cell | First run: runs, mean | Later runs: runs, mean |
|---|---|---|
| P1 | 4, $2.93 | 14, $0.53 |
| P2 | 4, $0.85 | 6, $0.22 |
| T1 | 12, $0.35 | 25, $0.08 |
| T2 | 4, $0.80 | 9, $0.11 |
| R1 | 14, $0.36 | 44, $0.32 |
| I1 | 13, $5.42 | 108, $0.80 |

In W22, R2 handed in a better way while the plan was new to P1 and P2. Under the pass, P1 ($2.86) and P2 ($0.44)
reviewed R1's first plan before R1 answered R2, and P1 then looked again ($0.49).

The query, run read-only on `store.sqlite`:

```sql
with r as (
  select st.work, st.run, st.cell,
         (select sum(usd_micros) from spent sp where sp.work = st.work and sp.run = st.run) / 1e6 as usd,
         row_number() over (partition by st.work, st.cell order by st.run) as n
  from started st
  where st.cell in ('P1', 'P2', 'T1', 'T2', 'R1', 'I1'))
select cell, case when n = 1 then 'first' else 'later' end, count(*), round(avg(usd), 2)
from r where usd is not null group by cell, n = 1;
```
