# The project's turn to check and merge, worked out from records: what the recorded works say

For: the plan of item #87 and its reviewers, who judge whether the turn can leave the acts' carrier and be worked out
from the works' records, as the walk works out a slot. Kept until that change is merged and its decision written; then
deleted, git keeping it. Measured on 2026-11-23 from the owner's store as it stood that day.

## Questions

1. How often did a work's check wait for the turn, a wait the bar now draws as the check running?
2. Does the order "first come, by when the check began to wait; at the same moment, the lower work first" give the
   order the carrier gave? Written before measuring (the `found` block of W87): a candidate is rejected if it names a
   different first work than the one that checked first, on any recorded case of two works due together.
3. Did the fifteen-minute limit on a turn kept after a passed check (`TURN_LEFT`) ever act, so that a rule worked out
   from records without a clock must keep it?

## Data and method

Every round of a run at M1 (the check) or M2 (the merge) of the project `wf-agents`, with its begin and end times, the
check's value and how many rounds the run had, read by the query below; `turn-in-the-walk.py` reads its result.

```sql
WITH t AS (SELECT s.work, s.run, s.cell,
    (SELECT MIN(r.at) FROM rounds r WHERE r.work = s.work AND r.run = s.run) AS began_at,
    (SELECT MAX(x.at) FROM returned x WHERE x.work = s.work AND x.run = s.run) AS ended_at,
    (SELECT COUNT(*) FROM rounds r WHERE r.work = s.work AND r.run = s.run) AS nrounds
  FROM started s WHERE s.cell IN ('M1', 'M2'))
SELECT t.work, t.run, t.cell, t.nrounds, i.project,
  (SELECT m.time FROM moments m WHERE m.work = t.work AND m.place <= t.began_at ORDER BY m.place DESC LIMIT 1) AS t0,
  (SELECT m.time FROM moments m WHERE m.work = t.work AND m.place <= t.ended_at ORDER BY m.place DESC LIMIT 1) AS t1,
  (SELECT w.value FROM writes w WHERE w.work = t.work AND w.run = t.run AND w.block = 'checked') AS checked,
  (SELECT COUNT(*) FROM moments m WHERE m.work = t.work AND m.place > t.began_at AND m.place < t.ended_at) AS inside
FROM t JOIN works k ON k.number = t.work JOIN items i ON i.number = k.item
WHERE t.began_at IS NOT NULL ORDER BY t0
```

The carrier records neither when a check took the turn nor which work held it. A check's round began as the walk came
to it and waited inside the round, so the round's begin is when the check began to wait. A check that began while
another work had a check or merge round under way waited, under first come, at least until that work's run of check
and merge rounds ended; that is the inferred wait below, capped at the check's own end.

Two periods: from 2026-11-14 12:53 UTC (`1794660814000`), when the turn came in (7b8ca89d8), and from 2026-11-15 04:00
UTC (`1794715244000`), when the order and the pause rule came in (13f2c6e60). The engine runs a commit some time after
it is made, so each period's first hours may still be on the code before it.

## Results

| | since the turn | since the order rule |
|---|---|---|
| checks | 114 | 97 |
| begun while another work's check or merge was under way | 78 | 66 |
| of them with one round: inferred wait, total | 64: 33,458 s | 52: 26,966 s (7.5 h) |
| longest inferred wait of one round | 1,584 s | 1,584 s |
| most moments inside one such round | 9 | 9 |
| of them with more rounds (a round failed and was tried again) | 14: 39,070 s | 14: 39,070 s |
| pairs of one-round checks in which the one begun later ended first | 16 | 0 |
| checks begun in the same millisecond, the lower work ending first | 4 of 4 | 4 of 4 |
| passed checks followed by more than a second without a check or merge round of their work | 5 | 0 |

The 16 pairs out of order are all in the first hours (works 5, 6, 7, 9 and 10, 2026-11-15 01:13 to 02:00 UTC), before
the order rule. In each, the check that ended first is either of a work that took the turn out of order, as the code
before that rule let it (works 6, 7 and 9, each paused while it waited), or a holder's second check after main moved
(work 5's run 55, work 7's runs 53 and 54, work 9's run 40), which the carrier gives the work that holds the turn. The
37 pairs out of order among runs of more rounds are each a
round that failed, let the turn go and waited for it again behind the works that had begun to wait meanwhile, as the
carrier does. The five long gaps after a passed check are all before the order rule, each of a work the owner paused
during its check (works 4, 5, 6 and 10) or of one that never merged (work 8).

## Conclusions, within what was tested

1. Waiting for the turn is common: since the order rule, 52 single-round checks waited about 7.5 hours in all, which the
   bar drew as the check running.
2. First come, by when the walk came to the check, with the lower work first at the same moment, agrees with every
   recorded case the carrier ordered by its own rules (no pause, no failed round). The candidate stands.
3. Since the order rule, no passed check was followed by more than a second before its work's next check or merge
   round: `TURN_LEFT` never acted. A work paused after its check passed gave the turn up by the pause rule, not by the
   limit.
4. At most nine moments fall inside one check's round, wait included, so working out since when a work has waited, by
   deciding at each of its moments back to where the wait began, costs a few decisions per waiting work.

Not tested: whether a round tried again should keep the turn; the records cannot say, as the carrier let the turn go at
every failed round.
