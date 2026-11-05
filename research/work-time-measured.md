# Where a work's time goes since the fast CI moved to the 14900k runners, measured on 2026-11-05

Item #305. The engine's data is read as it stood at 2026-11-05 13:30:27 UTC (queries Q2 to Q8 stop there, so a re-run
gives the same sets); GitHub's job records were read at about 14:00 UTC the same day. This report measures; it changes
no engine, CI or flow file. Whom it serves: the architect, who decides what to speed up next, and the owner, who decides
how fast a small fix must be. It is kept until the numbers are measured again after the next change to the CI or to the
cells' rounds.

Files, all in `research/`:

- `work-time-measured.md` (this report);
- `work-time-measured-queries.sql`: the queries on the engine's database, Q1 to Q9, as they were run with the engine's
  `query` tool;
- `work-time-measured.sql`: the GitHub job times of the runs in the group "after" and the SQL that turns them into the
  runner wait; run it with `sqlite3 -safe -readonly :memory: < research/work-time-measured.sql`.

Each claim says how it is known: (measured) on the data of the dates above, (read) from the source or the records,
(inferred) from measurements, not tried.

## 1. The answers

| Question | Answer |
|---|---|
| How many works, and how are they told apart | 24 works merged since the fast CI began on 2026-11-03 10:52 UTC. Three groups by where their CI ran: **before** (13 works, merged before the first run on a 14900k runner, 2026-11-04 23:58 UTC, all on hosted runners), **between** (5 works, some runs on each), **after** (6 works: 317, 328, 330, 331, 332, 334; every one of their 51 `ci`, `ci_before` and merge runs was read on GitHub and ran on a runner labelled 14900k). 5 of the 6 are small fixes (no deep research). The six merged between 01:02 and 01:59 UTC on 2026-11-05; nothing merged after that until the cutoff (measured). |
| Start to merge, after | All 6 works: median 39.4 min, sum 413.9 min. The 5 small fixes: median 26.2 min, sum 160.6 min (11.5, 14.9, 26.2, 52.6, 55.4 min). |
| What the time is made of, after | **Small fixes (5 works, 160.6 min):** agent rounds 51.8%, rounds that ran `ci` or `ci_before` 29.4% (19.2% the rounds' own time, 10.2% waiting for a runner), the merge round 8.2% (6.9% and 1.3%), waiting for an answer from the architect 7.7% (one work, 12.4 min), gaps between rounds 2.8%, the engine's own steps 0.1%. **All 6 works (413.9 min):** agent rounds 44.6%, gaps 35.7% (all in work 317, which waited for the owner's review of its design and at the merge gate, and for a free slot), `ci` rounds 12.8%, merge round 3.7%, answers 3.0%. Runner wait is 11.5% of the small fixes' time (18.4 of 160.6 min) and 5.1% of all six (21.0 of 413.9 min). Section 3 has each class in median, sum and share. |
| The cells that took most time | In the six works: `I1` Implement 59.0 min in 33 rounds (608 model calls), `X1` Run tools 53.3 min in 65 rounds (CI), `D1` Deep research 33.7 min in 5 rounds, `R1` Initial research 24.5 min in 16 rounds (191 calls), `D2` 22.5 min in 3 rounds, `M2` the merge 15.3 min in 10 rounds. Section 6. |
| Model time | **Not recorded**, so not measured: a round's record holds the CLI's total time (which is the round's time and includes the commands it ran), the number of model calls and the tokens, not the time spent on the model. A fit of time from calls and tokens was tried by a rule written before it was looked at and **failed it** (section 6): the report gives each cell's round time, which is the most the model took, and its model calls. |
| Small fixes against works 302 and 316 | Works 302 (41.8 min) and 316 (24.4 min) are in the group "before", with 4 other small fixes: median 51.3 min (24.3 to 102.5 min). After: median 26.2 min (11.5 to 55.4 min). 23 of the 30 pairs (an after against a before work) are shorter after. **Not decided** with 5 against 6 works: the ranges overlap and the works were different tasks. What moved is measured per work in the rounds: the median `ci` rounds went from 12.1 to 3.4 min, the merge round from 5.4 to 2.5 min; the median agent rounds went from 22.8 to 18.9 min. Section 7. |
| Runner wait | After the move, a `ci` or `ci_before` call that was the longest of its round waited 37.4% of its time for a runner (1132 of 3025 s in 30 calls, the longest 195 s); a merge's full CI waited 14.9% (128 of 859 s in 7 calls). Rounds that asked for `ci` and `ci_before` together waited 53.8 s on average and ran 122 s; those that asked for one waited 25.5 s and ran 85 s. In 11 of 51 runs the run's last job, which does 3 to 5 s of work, waited 60 to 111 s for a runner. Section 5. |

## 2. How the numbers were taken

- **The move to the 14900k runners is not one instant** (read and measured). `.github/workflows/fast.yml` was changed to
  `runs-on: [self-hosted, 14900k]` by commit 44e32231 on main (2026-11-04 23:36 UTC), and a work's runs use the file its
  branch holds, so each work moved when it next merged main. Run 41937774408 (2026-11-04 23:58 UTC, work 309) was the
  first read on a 14900k runner, and run 41938479809 (2026-11-05 00:03 UTC, work 313) was still on `ubuntu-24.04`. The
  `ci` calls of 200 to 300 s give way to calls mostly under 150 s between 00:03 and 00:20 UTC (Q9). The report therefore groups by
  where each work's runs were, not by a time: **after** is a merged work whose first `ci` of 10 s or more started at or
  after 00:20 UTC on 2026-11-05, and the labels of all 51 runs of those works were read (51 on 14900k). The 3 runs read of
  works 324, 323 and 308 were on hosted runners, which is why those works are **between**.
- **This is not the switch #282 measured.** `research/ci-speed-measured.md` calls the first `ci` with named tests, on
  2026-11-03 10:52 UTC, the switch; that moved the CI to the fast CI on hosted runners. The 14900k runners came a day
  later. This report's sets start at that first moment (works whose first round started at or after it) and split
  at the second. Works 302 and 316, which the item gives as before the switch, are in the group **before**.
- **A work's span** is the start of its first round to the end of the round that merged (the round of its `merge` call
  whose result is a commit id), as the engine records them. The item's figures for works 302 and 316 (41.8 and 24.4 min)
  run to the start of the work's last round (`E1`, after the merge); to the merge they are 41.6 and 24.3 min (Q1).
  Both are given in Q1; the report uses the span to the merge.
- **Classes of a span** (Q2; they do not overlap, and a check of each work's span less the sum of its classes is 0 s,
  with no rounds overlapping, measured): `agent` rounds (a round that ran a model); `ci` rounds (cell `X1` and others
  whose tool calls are `ci`, `ci_before`, `full_ci`, `measure` or `watch_main`); the `merge` round (`M2`); waiting for
  an answer (cells `A2` to the architect and `A3` to the owner); the engine's own steps and rounds that ran only `query`
  or `gh`; and the **gaps** between rounds (the next round's start less the previous round's end). The part of a gap that
  overlaps a row of `slot_waits` is a wait for a free slot. The engine writes a tool call's `queued_at` and `started_at`
  as one moment (886 of 886 calls of `ci` equal when #282 measured it, and `attempts` in `crates/engine/src/tools.rs`),
  so the table cannot show a wait for a runner: it is read from GitHub.
- **Runner wait** is read from GitHub's jobs of each run (`gh api repos/<repo>/actions/runs/<run>/jobs`): a job's wait is
  its `started_at` less its `created_at`. A run's wait on its critical path is the wait of its last job plus the wait of
  the other job that ended last (the jobs `lint` and `test` start together; the last job `fast`, or `check` of the full
  CI, starts when both end). For a round, the longest call of the round is taken, and only the first call that used a run
  (a merge that looks at the same run again waits for nothing). It is taken out of the round's time, so that the classes
  stay exclusive: the `ci` rounds and the merge round below are less their runner wait. The data is in
  `work-time-measured.sql`, kept because GitHub keeps job records for 90 days only.
- **What a work with a deep-research round is**: a work that had a round at cell `D1`. `work_routes` agrees for every
  work but 287, which has no route recorded and no `D1` round (it is counted a small fix).
- **Medians** are of the per-work values; with 5 or 6 works they are the middle value or the mean of the two middle ones,
  and no interval is given (an interval of a median of 5 works spans nearly the range). The sets are small and are
  written with their number every time.

## 3. Where a work's time goes, in the group "after"

Minutes per work, from Q2 and `work-time-measured.sql` (rounded to 0.01 min, so a row adds up to its span within 0.01).
Work 317 is the only one with deep research.

| Work | Span | Agent rounds | `ci` rounds, less wait | Merge round, less wait | Runner wait | Waiting for an answer | Gap: slot | Gap: other | Engine steps, light tools |
|---|---|---|---|---|---|---|---|---|---|
| 317 (designed) | 253.35 | 101.63 | 3.35 | 2.12 | 2.62 | 0.00 | 42.99 | 100.44 | 0.21 |
| 328 | 52.62 | 29.01 | 14.38 | 2.17 | 5.44 | 0.00 | 0.00 | 1.58 | 0.03 |
| 330 | 55.36 | 18.86 | 9.50 | 3.83 | 9.10 | 12.44 | 0.01 | 1.57 | 0.06 |
| 331 | 26.17 | 18.90 | 1.42 | 1.95 | 3.45 | 0.00 | 0.00 | 0.40 | 0.05 |
| 332 | 11.50 | 6.72 | 2.82 | 1.24 | 0.25 | 0.00 | 0.00 | 0.43 | 0.05 |
| 334 | 14.93 | 9.67 | 2.71 | 1.83 | 0.15 | 0.00 | 0.00 | 0.52 | 0.05 |

Median and sum of each class, and its share of the sum of the spans (Q3 and `work-time-measured.sql`; measured):

| Class | 6 works: median | sum | share | 5 small fixes: median | sum | share |
|---|---|---|---|---|---|---|
| Span | 39.39 | 413.9 | 100% | 26.17 | 160.6 | 100% |
| Agent rounds | 18.88 | 184.8 | 44.6% | 18.86 | 83.2 | 51.8% |
| `ci` rounds, less runner wait | 3.08 | 34.2 | 8.3% | 2.82 | 30.8 | 19.2% |
| Merge round, less runner wait | 2.04 | 13.1 | 3.2% | 1.95 | 11.0 | 6.9% |
| Runner wait (in `ci` and merge rounds) | 3.04 | 21.0 | 5.1% | 3.45 | 18.4 | 11.5% |
| Waiting for an answer (`A2`, `A3`) | 0.00 | 12.4 | 3.0% | 0.00 | 12.4 | 7.7% |
| Gap: waiting for a slot | 0.00 | 43.0 | 10.4% | 0.00 | 0.0 | 0.0% |
| Gap: other | 1.04 | 104.9 | 25.4% | 0.52 | 4.5 | 2.8% |
| Engine steps and light tools | 0.05 | 0.5 | 0.1% | 0.05 | 0.2 | 0.1% |

In the four classes of the item's example (agent rounds, tools, gaps, merge) the five small fixes are: agent rounds 51.8%,
tools 29.4% (the `ci` rounds with their runner wait), gaps 2.8%, merge 8.2% (with its wait), and 7.7% waiting for an
answer, which the example did not have. The example's figures (52%, 26%, 12%, 10%; 7 works, 441 minutes) are from a
snapshot of works before the move (observation 253) and were not measured again here.

The groups before and between, for comparison (Q3; measured; the runner wait is not split out of their `ci` and merge
rounds, since their runs were read only in part):

| Group | Works | Span: median, sum | Agent rounds | `ci` rounds | Merge round | Gaps |
|---|---|---|---|---|---|---|
| before, all | 13 | 81.2 min, 1493.6 min | 44.5% | 27.1% | 5.5% | 22.7% |
| before, no deep research | 6 | 51.3 min, 331.5 min | 49.8% | 30.8% | 10.0% | 8.9% |
| between, all | 5 | 250.5 min, 937.8 min | 46.1% | 21.8% | 1.8% | 25.8% |
| between, no deep research | 2 | 53.2 min, 106.3 min | 53.2% | 38.5% | 6.2% | 1.6% |

## 4. The gaps between rounds

Q4, Q5 and Q6 (measured, with the class of each gap inferred from the cells it lies between and from when it ended):

- **After, small fixes:** 154 gaps, all under 60 s, 4.5 min together, 1.75 s each on average. Nothing longer: no wait
  for a slot, no engine stop.
- **After, work 317:** 4 gaps from `D3` to `D3` (waiting for the owner's review of its design) 50.6 min, of which 43.0 min
  overlap a wait for a slot; one from `M1` to `M1` (the merge gate, which a change to the flow passes only with the
  owner's approval, per the project guide) 46.3 min; one engine stop of 2.3 min; 53 gaps under 60 s, 1.2 min.
- **Before, small fixes:** 4 gaps of 60 s or more that end within 10 s of another work's gap of 60 s or more, 24.5 min
  together, and 153 gaps under 60 s, 3.0 min. The ends are at 2026-11-04 21:18:52 to :54 (works 312, 315, 316: 297, 305
  and 342 s) and 22:29:56 to 22:30:03 (works 308, 309, 319, 321: 505 to 593 s): several works resuming together after a
  gap of 5 to 10 minutes is the engine not running, which an installation of a new build does (inferred: the records
  have no row for it; the brief of this item says the engine stops for an installation).
  Work 316's 24.4 minutes hold one of these, 5.7 min between `R0` and `R1`; without it the work is 18.5 min.
- Between: 9 gaps of that kind, 41.7 min, and `D3` to `D3` waits of 140.5 min.

So the gaps between rounds are not a cost of a small fix when the engine runs: 1.75 s a gap. What was a cost was the engine
not running (about 25 minutes in four of the six small fixes of the group before, none in the group after), and the
owner's wait and the wait for a slot, which only a work with a design has.

## 5. Runner wait

Measured from the GitHub jobs of the 51 runs of the group "after" (30 `ci` and `ci_before` calls that were the longest
of their round, and 7 merge calls; all 44 `ci` and `ci_before` calls that name a run, and the full CI of each merge, were
read: of the 30 `ci` calls of these works, 28 name a run, the other two did no push).

| | Calls | Seconds | Runner wait, seconds | Share | Median call | Longest wait |
|---|---|---|---|---|---|---|
| `ci` or `ci_before` (longest in its round) | 30 | 3025 | 1132 | 37.4% | 86 s | 195 s |
| Merge (the full CI) | 7 | 859 | 128 | 14.9% | 121 s | 90 s |

| Round | Rounds | Mean wait | Longest wait | Mean call |
|---|---|---|---|---|
| `ci` and `ci_before` together | 13 | 53.8 s | 195 s | 122.0 s |
| One of them | 17 | 25.5 s | 113 s | 84.6 s |

By work, the minutes: 317 2.62, 328 5.44, 330 9.10, 331 3.45, 332 0.25, 334 0.15. Works 328 and 330, which asked for
`ci_before` in 6 and 7 rounds, hold 14.5 of the 21.0 minutes.

What this shows and does not show:

- The wait is a third of the time of a `ci` call on these runners (measured). The calls ran 31 to 276 s, so the wait
  is the largest part of the slow ones.
- The runs of the sample were served by runners named `14900k`, `14900k-2`, `14900k-3` and `14900k-4` (read from the jobs
  read on 2026-11-05 00:03 to 00:20 UTC): at least four, and a round with `ci` and `ci_before` together makes six jobs.
  That the wait is higher when both are asked (53.8 s against 25.5 s) fits the runners being the limit, but the two kinds of
  round are different works at different times, 13 against 17 rounds, so it is **not decided**; a test would put the
  two on the same machine in pairs.
- The last job of a run (`fast`, or `check` of the full CI) only waits for the others and does 3 to 5 s of work, but it
  needs a runner of the same pool: in 11 of the 51 runs it waited 60 to 111 s (read from the jobs). Whether it could run
  elsewhere without losing what it is for is **not tried**.
- The wait for runners on the hosted runners before the move was not read here; #282 read a mean of 3 s for the test
  job of 38 sampled `ci` calls on 2026-11-03 to 04.

## 6. The cells that took most time, and model time

Q7 (measured). The model calls (`turns`) are the CLI's count in the round's record; a cell that runs no model has none.
**The time spent on the model is not in the records.** `rounds.usage` holds the CLI's total time (equal to the round's time:
for a round of work 316: 100.4 s against 100 s) with the commands the round ran, the number of model calls and
the tokens, and nothing of the time of the model's API. A round's time is therefore the most the model took in it, and an
agent round with many calls includes the time of its Bash, Read and other commands.

The 12 cells of the six works that took most time are below; together they hold 15787 of the 15960 s the six works spent
in rounds (measured; the gaps between rounds are not in rounds).

| Cell | Rounds | Seconds | Median round | Model calls | Calls a round |
|---|---|---|---|---|---|
| `I1` Implement | 33 | 3541 | 41.1 s | 608 | 18.4 |
| `X1` Run tools (`ci`, `ci_before`, with the runner wait) | 65 | 3200 | 30.1 s | none | none |
| `D1` Deep research | 5 | 2021 | 481.2 s | 51 | 10.2 |
| `R1` Initial research | 16 | 1472 | 54.7 s | 191 | 11.9 |
| `D2` Think again outside the frame | 3 | 1351 | 500.1 s | 10 | 3.3 |
| `M2` The merge | 10 | 917 | 108.8 s | none | none |
| `T1` Adversarial testing | 15 | 760 | 13.1 s | 126 | 8.4 |
| `A2` To the architect (a wait for the answer) | 1 | 746 | 746.2 s | none | none |
| `R0` Gather the background | 6 | 541 | 82.1 s | 126 | 21.0 |
| `T6` Fix | 14 | 470 | 15.9 s | 106 | 7.6 |
| `T2` Review: the upper level | 8 | 395 | 41.3 s | 74 | 9.3 |
| `T3` Review: the details | 10 | 373 | 20.3 s | 48 | 4.8 |

For all 24 merged works since the fast CI (Q7): `X1` 36865 s in 342 rounds, `I1` 22887 s in 123 rounds (3170 model calls,
25.8 a round), `D1` 10717 s in 32 rounds (747 calls), `R1` 9232 s in 68 rounds (1116 calls), `T2` 7937 s in 44 rounds (561
calls; 2976 s of it in rounds that ran no model), `M2` 6878 s, `T6` 6206 s in 126 rounds (1311 calls), `T1` 5849 s,
`D2` 5107 s, `T3` 3695 s.

**The estimate of model time failed its rule** (Q7b and Q7c). Before looking at any result, the plan was to fit the
seconds of a round as `a` times its model calls plus `b` times its output tokens, for each model, on the rounds that
called the model at most 3 times (so almost no command time is in them), and to use it only if on the rounds of more than
3 calls the fit was above the real time in at most 10% and the median of real less fitted was not negative. The fit on the
rounds of at most 3 calls is as it should be (fitted over real 0.99 to 1.03 for the four models), but on the rest it
is above the real time in 70.4% of the rounds for opus (860 fitted, 477 tested), 76.7% for sonnet (366 and 270), 85.5%
for haiku (55 and 83) and 95.8% for fable (16 and 24), and the median of real less fitted is negative for all four
(-5.1, -7.1, -17.0 and -277.3 s). It overestimates: the cost of a model call is not the same in a round of three calls and a
round of twenty. So **no model time is given**; the column `est_model_s` of Q7 is the rejected fit and is not used.
Recording the API time per round would give it directly (observation 277).

## 7. Small fixes against works 302 and 316

Q1 and Q3 (measured). Small fixes: works without a `D1` round.

| | Works | Span to the merge, each | Median |
|---|---|---|---|
| Before (hosted runners) | 6: 287, 302, 312, 315, 316, 319 | 35.2, 41.6, 102.5, 67.0, 24.3, 61.0 min | 51.3 min |
| After (14900k runners) | 5: 328, 330, 331, 332, 334 | 52.6, 55.4, 26.2, 11.5, 14.9 min | 26.2 min |

Work 302 took 41.8 min to its last round (41.6 to the merge), work 316 24.4 min (24.3). Work 330's 55.4 min include 12.4
min that it waited for the architect's answer (cell `A2`); without it the median of the five is 26.2 min as well.

| Median of the small fixes, minutes | Before (6 works) | After (5 works) |
|---|---|---|
| Span | 51.32 | 26.17 |
| Agent rounds | 22.78 | 18.86 |
| `ci` rounds (with the runner wait) | 12.11 | 3.37 |
| Merge round (with the runner wait) | 5.38 | 2.49 |
| Gap: other | 5.75 | 0.52 |

Of the 30 pairs of an after work against a before work, 23 are shorter after (measured). With 5 and 6 works that is
**not enough to say the median moved**: the ranges overlap (24.3 to 102.5 against 11.5 to 55.4 min), the tasks differ, and
other things changed in the same hours: research and review ran at effort high from 2026-11-05 00:02 UTC (commit f86ef835),
which could lengthen agent rounds, and four engine stops (24.5 min) fell in the before group. What is measured is that the
rounds of the tools shortened (the medians of the `ci` rounds and the merge round add up to 17.5 min before and 5.9 min
after) and the gaps between rounds, while the agent rounds, the largest class, did not change by more than the spread of the works.

## 8. What was not measured, and the limits

- The sets are small and from one night: 6 works in the group after (5 small fixes), 13 before (6 small fixes). Nothing
  merged after 2026-11-05 01:59 UTC until the cutoff at 13:30 UTC. Re-run the queries when more works have merged.
- Model time (section 6) and the time an agent round spent in its commands are not in the records.
- The runner wait is read for the group after only; the groups before and between have their `ci` rounds with the wait in
  them. All runs of the works of the group after were read, but not the runs of the two `ci` calls that did no push.
- The classes of a gap in section 4 are read from the cells and from when the next round started, not from a record of what
  the engine did; an engine stop is inferred from several works resuming within 10 s.
- The time after the merge (the learning round, waiting for a change to take effect) is not in the span; work 334's merge
  was followed by 11.5 hours before the next round (`L2`), which the engine's records do not explain.
- The old whole CI before the fast CI is not measured again here; item #305's 7 works and 441 minutes stay a figure of
  observation 253.

## 9. To reproduce

- `research/work-time-measured-queries.sql` holds Q1 to Q9; run each with the engine's `query` tool, one at a time. Q2
  to Q8 stop at the cutoff and give the numbers above; Q1 and Q9 have no cutoff.
- `research/work-time-measured.sql` holds the GitHub job times and the sums of the runner wait: run it with `sqlite3 -safe
  -readonly :memory: < research/work-time-measured.sql`; it is one statement, so it also runs with the `query` tool. It
  needs no GitHub. The job times were written by the command in the last comment of the queries file, one for each run
  of Q8.

## 10. What to do next

1. **Runner wait is the one part of the CI time left that is not compiling or testing**: 11.5% of a small fix and a third of
   the time of a `ci` call. Ask which of three would shorten it, and measure the one chosen in pairs against a twin, as the
   rules for performance ask: more runners on the machine; the last job (`fast`, `check`) off the 14900k pool; the engine
   not asking for `ci` and `ci_before` in the same round when it need not. None of the three is tried.
2. **The agent rounds are 45 to 52% of a work and do not move with the CI**: `I1` alone took 59 minutes of the 414. Where
   they are shortened is a matter of the rounds (their number, their calls, what they are given), not the CI. The time of the
   model in them needs the API time recorded (observation 277) before it can be told from the time of commands.
3. **The gaps and waits that remain are those of the engine not running and of the owner and the slots**, 2.8% in a small fix
   when the engine runs. The engine stops (about 25 minutes in four of six small fixes before) are a matter of how a new
   build is installed; the owner's waits at `D3` and `M1` and the wait for a slot are for works that have a design.
4. **Re-measure** with the same queries when 10 or more small fixes have merged on the 14900k runners. The owner's target for
   a small fix is still open: this report gives a median of 26.2 min over 5 works, with 18.9 of them in agent rounds.
