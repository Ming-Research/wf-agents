# What the fast CI costs, measured on 2026-11-03 and 2026-11-04

Item #282. The engine's data is read as it stood at 2026-11-04 21:18:53 UTC (every query but Q1b and Q10 stops there,
so a re-run gives the same sets); GitHub's records were read at about 21:20 UTC the same day. This report measures; it changes no engine, CI or flow
file. Whom it serves: the architect, who decides what to change next in the CI, and the reviews of #244, #251 and #231,
whose acceptance numbers it measures. It is kept until the numbers are measured again after the next change to the CI.

Files, all in `research/`:

- `ci-speed-measured.md` (this report);
- `ci-speed-measured-queries.sql`: the queries on the engine's database, Q1 to Q10, as they were run with the engine's
  `query` tool;
- `ci-speed-measured.sql`: the job times read from GitHub for the sampled runs, and the SQL that sums them; run it
  with `sqlite3 -safe -readonly :memory: < research/ci-speed-measured.sql`.

Each claim says how it is known: (measured) on the data of the dates above, (read) from the source or the workflow
files, (inferred) from measurements, not tried.

## 1. The answers

| Question | Answer |
|---|---|
| `ci` duration, median, before and after the switch to the fast CI | Before: 407 s, 95% interval 405 to 423 s (470 calls, whole CI, 2026-10-26 to 2026-11-03 09:21 UTC). After: 218 s, interval 204 to 220 s (161 calls, named tests, 2026-11-03 10:52 UTC to 2026-11-04 21:19 UTC). The target is 180 s: **not reached**; 19 of the 161 calls (12%) took 180 s or less. |
| What the 218 s are made of | On the mean call of 38 sampled ones (216 s): building the page 63 s (29%), compiling and running the tests 53 s (24%), setting the job up 26 s (12%), restoring main's build state 20 s (9%), the engine before and after the run 27 s (12%), the lint job ending after the tests 12 s (6%), waiting for a runner 3 s (1%). Running the tests themselves is 3 to 7 s (about 2%). |
| What a work waits for in CI | The 25 works merged last before the switch: mean 93.7 min, median 90.5 min. The 7 works that started after the switch and are merged: mean 60.1 min, median 44.5 min (49.3 and 37.5 min when the time `ci` and `ci_before` overlap counts once). The target of 34.8 min is met by 2 of the 7 works (3 of 7 on the second count). **Undecided**: 7 works, and the intervals overlap. The baseline of 69.7 min is reproduced from the records (section 5) as a snapshot of works not yet finished, taken at about 2026-11-03 04:37 UTC; the same 25 works now come to 88.9 min. |
| The full CI green the first time | Before the switch 42 of 43 works (98%); after it 10 of 12 (83%). Not told apart: the intervals (88% to 100%, 55% to 95%) overlap. |
| `ci` and `ci_before` together | 59 rounds ran the two side by side. The round lasts 3.53 min against 3.50 min for `ci` alone: `ci_before` costs the round 2 s and the overlap saves 2.82 min a round, the whole of the shorter run. No lengthening of either run or of the wait for a runner is detected. |
| What is worth changing | Building the page: about 58 s a call and the page is the same unless `client/` changed (inferred, not tried). Then the engine's 20 s polling, about 8 s a call (inferred). The tests, the runner wait and the lint job (already moved off the critical path) are not worth changing. Whether main's build state is worth restoring is undecided (section 4) and needs paired runs. Section 7 has the arithmetic, and what is still needed to reach 34.8 min. |

## 2. How the numbers were taken

- The engine records one row of `tool_runs` for each run of `ci` or `ci_before`. It writes `queued_at` and
  `started_at` as one instant (`attempts` in `crates/engine/src/tools.rs`), so the table has the duration of a call and
  nothing of its parts (read). The parts come from GitHub: the jobs and steps of each run, with their times to the
  second (`gh api .../actions/runs/<run>/jobs`) and the logs of seven jobs.
- A `ci` call is a row of `tool_runs` with the tool `ci`, a push and 10 s or more; the shorter ones are refusals and
  calls that only hand in the pull request's text. Its duration is the engine's, from the start of the tool to its end
  (the tool time). GitHub's own time for the run is given beside it where it was read (the run time).
- The switch is 2026-11-03 10:52:28 UTC, the first `ci` that ran named tests (`push.run` is set in its result). Before
  it the CI ran the whole test set on every push. On 2026-11-03 07:38:24 UTC the workspace's own crates began to be built
  unoptimized (commit 3a9ea8d9); a work took that up when it next merged main. Calls between that moment and the switch
  are set apart as group B, to see whether it alone changed the duration.
- A median comes with the interval from the ranks of the sorted values: the values at ranks
  j = floor((n - 1.96 sqrt n) / 2) and k = ceil(1 + (n + 1.96 sqrt n) / 2). It needs no assumption about the shape of the
  data. Two medians whose intervals overlap are reported as not told apart.
- This is a comparison of two periods, not an experiment: no interleaved pairs and no twin of the base were run, and no
  measurement workflow is listed in the project's configuration (`project.ci.measure` is none). What is said about a cause
  is said to be inferred.

## 3. `ci` before and after the switch

| Calls | n | median | 95% interval of the median | mean | min to max | at most 180 s |
|---|---|---|---|---|---|---|
| A: whole CI, 2026-10-26 10:54 to 2026-11-03 07:38 UTC | 440 | 407.7 s | 404.2 to 423.6 s (ranks 199, 242) | 368.8 s | 10.0 to 768.1 s | 33 |
| B: whole CI, unoptimized crates in main, to the switch | 30 | 407.1 s | 391.2 to 429.2 s (ranks 9, 22) | 404.3 s | 202.8 to 487.9 s | 0 |
| A and B together | 470 | 407.1 to 407.7 s (between the two medians) | 404.9 to 422.8 s (ranks 213, 258) | | | 33 |
| C: named tests, from the switch | 161 | 218.3 s | 204.5 to 219.5 s (ranks 68, 94) | 212.5 s | 57.9 to 265.5 s | 19 |

(measured; Q1, Q2.) The median fell from about 407 s to 218 s, by 46%. Group B alone shows nothing (407.1 s against
407.7 s), so the unoptimized build is not the cause of the fall; what else changed at the switch is the workflow that
runs, which no longer compiles and runs the whole test set on three shards (read: the old logs run `cargo nextest run
--partition count:$SHARD/3`). That reading is inferred; the old workflow was not run again on the new days to check it.

Against the target of 180 s: the lower end of the interval, 204.5 s, is 24.5 s above it; the median is 38.3 s above it.
GitHub's run time agrees in direction: for the runs of `fast.yml` on the work branches, completed between 2026-11-03
10:52 and 2026-11-04 21:19 UTC, the median is 196 s (mean 190.0 s, 39 to 244 s, 162 runs), 16 s above the target;
the 38 sampled runs have median 196.5 s with interval 174 to 206 s, which contains 180 s, so on the run time alone the
target is not decided. The 162-run figure was read once, with `gh run list` (section 9), and its list is not kept in the
repository: the command lists the latest runs and GitHub keeps them 90 days, so a later re-run gives other runs. Only
the figures of the 38 sampled runs, whose times are in `ci-speed-measured.sql`, can be recomputed. The tool time, which is what #244 counts (the `ci` call the engineer waits for), is above it.
By what a call asked for (Q1b, read at 2026-11-04 21:03 UTC, so with a few calls fewer): named tests with the lint
218.9 s (123 calls), named tests alone 215.7 s (7), named tests with a named check 215.5 s (18); the calls that ran only
the lint 199.2 s (11). A call with the lint alone lasts about as long as one with the tests, so the lint job is as long
as the test job (about 170 s once the engine's 27 s are taken off; inferred), which is why it sometimes ends last.

How often a call is red: 60 of the 161 named calls (37%) and 211 of the 470 whole-CI calls (45%) had a red check
(measured; Q9).

## 4. Where one `ci` call spends its time

The sample is 38 `ci` calls (every fourth call with a test job, apart for the calls with and without a `ci_before` in
their round: 14 and 24) and 22 `ci_before` calls (all 11 without a `ci` in the round, 11 of the 68 with one). Each
segment is a step or a job of the run; the segments follow one another from the start of the engine's tool to its end,
and they add up to the tool time within 1 s (2 of the 60 rows are off by 1 s; section 3 of the SQL file). The mean call
of the sample is 216.2 s. The sample is like the whole: its median tool time is 219 s (interval 203 to 238 s) against
218.3 s for all 161 calls, and its median run time 196.5 s against 196 s for all 162 runs.

| Segment of a `ci` call | mean | median | share of the mean call | what it is |
|---|---|---|---|---|
| Engine, before the run exists | 7.2 s | 7 s | 3.3% | push, pull request, dispatch of the workflow |
| Waiting for a runner | 3.1 s | 3 s | 1.4% | the test job queued |
| Setting the job up | 26.2 s | 25 s | 12.1% | job set-up, checkout, toolchain, nextest, rust-cache, node |
| Restoring main's build state | 19.8 s | 20 s | 9.2% | `actions/cache/restore` of `target/ci` |
| Building the page | 63.1 s | 68 s | 29.2% | `npm ci` and the vite build |
| Compiling and running the tests | 52.8 s | 44 s | 24.4% | the step `Test` |
| Page tests | 0.3 s | 0 s | 0.1% | only when page tests are named |
| Rest of the test job | 4.6 s | 4 s | 2.1% | upload of results, closing steps |
| Lint ending after the tests | 12.2 s | 0 s | 5.7% | the lint job was still running when the test job ended |
| Summary job `fast` | 7.2 s | 6 s | 3.3% | |
| Engine, after the run ended | 19.7 s | 18 s | 9.1% | polling, reading the jobs and the results |
| The tool run | 216.2 s | 219 s | 100% | |

(measured; section 1 and 2 of the SQL file.) For `ci_before` (22 calls, mean 159.8 s): the engine before 6.0 s, runner
wait 3.1 s, set-up 24.4 s, restore 23.2 s, page 57.8 s (36%), compile and run 14.3 s, rest 3.4 s, summary 6.7 s, the
engine after 20.9 s (13%). It has no lint, and its `Test` step is short because few tests are named.

What stands behind the larger segments:

- **The page.** `Build the page` is the biggest segment of `ci` and of `ci_before`. In seven logs the vite build took
  33.7 to 40.4 s and `npm ci` 6 to 46 s. The step is the same code on every call that does not change `client/`
  (read: `.github/workflows/fast.yml`, the step before the cargo steps, as the crate `web` embeds the built page).
- **Compile and run.** Of seven job logs, the tests ran in 0.9 to 7.1 s (median 4.0 s for the six `ci`) and cargo's own
  line `Finished` gave the compile: 25.3, 36.1, 46.1, 36.5 and 37.3 s with main's build state restored, 72 s in the one
  run where it was not found (measured; the table below). The step is 90% compile.
- **The lint job.** On 2026-11-03 every call ran the lint; in 11 of the 31 sampled calls of that day (35%) it ended
  after the test job, by 42.3 s on the mean, and those calls lasted that much longer. Since 2026-11-04 the lint runs only
  when a call names it, and in the 7 sampled calls from then (5 named it) it ended before the test job every time
  (measured). The 5.7% is gone for the calls that do not name the lint; for those that do it may come back.
- **The engine's own 27 s** (12.4% of `ci`, 16.8% of `ci_before`). The engine looks at a running workflow every 20 s
  (read: `poll` in `Gh::new`, `crates/engine/src/hub.rs`). The time after the run ended ranges from 8 s to 53 s with
  median 18 s: a floor of about 8 s for reading the jobs and the results, and, if the run ends at a random moment in a
  20 s wait, 10 s more on the mean (inferred from the shape: the floor and the mean match; not isolated).

| Job (log) | Day, UTC | Restored build state | `npm ci` | vite | Compile (`Finished`) | Tests run (nextest) |
|---|---|---|---|---|---|---|
| `ci` 125883236415 | 11-03 | 1162 MB | 35 s | 39.6 s | 25.3 s | 5.3 s (2 tests) |
| `ci` 126053389596 | 11-03 | 1221 MB | 7 s | 40.4 s | 36.1 s | 7.1 s (5) |
| `ci` 126094614183 | 11-03 | 1265 MB | 6 s | 33.7 s | 46.1 s | 3.2 s (2) |
| `ci` 126814006399 | 11-04 | 2001 MB | 46 s | 39.3 s | 36.5 s | 4.0 s (7) |
| `ci` 126820048668 | 11-04 | 2001 MB | 46 s | 40.2 s | 37.3 s | 4.0 s (17) |
| `ci` 126850919554 | 11-04 | not found | 46 s | 39.6 s | 72 s | 2.7 s (2) |
| `ci_before` 126848861834 | 11-04 | 2040 MB | 46 s | 39.2 s | 10.5 s | 0.9 s (1) |

(measured; a job log of an eighth run, 125963626847, stopped in the compile and is not used.) The restored build state
doubled from about 1.2 GB on 11-03 to 2.0 GB on 11-04.

**Is main's build state worth restoring?** In the 38 sampled `ci` calls, 31 restored it and 7 did not find a state to
restore (the key holds the hash of `Cargo.lock`, and a work that changes the lock file finds none):

| Calls | n | restore | `Test` step | restore and `Test` | median of that |
|---|---|---|---|---|---|
| restored | 31 | 24.2 s | 47.8 s | 72.0 s | 66 s |
| not found | 7 | 0.1 s | 74.9 s | 75.0 s | 76 s |
| restored, 11-03 | 26 | 21.4 s | 49.3 s | 70.7 s | |
| restored, 11-04 | 5 | 39.0 s | 39.8 s | 78.8 s | |
| not found, 11-03 / 11-04 | 5 / 2 | 0.2 / 0 s | 75.2 / 74.0 s | 75.4 / 74.0 s | |

(measured; section 5 and 5b of the SQL file.) Restoring saves about 27 s of compile and costs 24 s. Over all 38 calls
the restore and `Test` steps take 3 s less on the mean (72.0 against 75.0 s) and 10 s less on the median (66 against
76 s) when the state was restored; on 11-03 they take 4.7 s less (26 against 5 calls) and on 11-04 4.8 s more (5
against 2 calls), when the restored state had grown from 1.2 to 2.0 GB. So the observations lean to keeping the restore
and only the two last days of them lean the other way: **undecided**. The two groups are not paired: the cause of each
of the 7 misses was not read one by one (the key holds the hash of `Cargo.lock`, so a work that changes the lock file
finds none, and so may any work just after main changed it, before main's CI saved the new key), and a call that
changes the lock file also changes more code and compiles longer, which lowers the gain of the restore. 7 calls do not
carry a difference of a few seconds. It needs paired runs (same commit, with and without the restore) before the restore
is kept or dropped.

## 5. What a work waits for

The wait of a work is the sum of the durations of its calls of `ci`, `ci_before`, `full_ci` and `merge`: the
baseline's definition (read: #38's description). The second count takes the union of the intervals of the four tools, so
that the time `ci` and `ci_before` run side by side counts once. `merge` includes the full CI that runs inside it.

| Works | n | wait, mean (median) | union, mean (median) | `ci` calls, mean (median) | minutes a `ci` call |
|---|---|---|---|---|---|
| PRE25: the 25 merged last before the switch (2026-11-02 21:02 to 11-03 09:10 UTC) | 25 | 93.7 (90.5) min | the same | 13.6 (13) | 5.30 |
| POST: first `ci` after the switch and merged | 7 | 60.1 (44.5) min | 49.3 (37.5) min | 9.3 (7) | 3.63 |

(measured; Q5, Q6, Q6b.) PRE25's median has interval 61.1 to 129.9 min (ranks 7, 19: works 261 and 231). POST's
interval is its whole range, 16.6 to 162.8 min, since n = 7 gives no narrower one by the rank rule. They overlap:
**undecided** on the median. The mean fell by 36%, the median by 51%; one work, 280, made 27 calls and waited 162.8 min.

| Work | First `ci` | Merged | `ci` calls | Wait | Union | `ci` | `ci_before` | Full CI | `merge` |
|---|---|---|---|---|---|---|---|---|---|
| 281 | 11-03 13:28 | 11-03 14:52 | 6 | 39.8 | 37.5 | 20.7 | 5.3 | 13.7 | 0.1 |
| 287 | 11-03 15:52 | 11-03 16:09 | 1 | 16.6 | 13.9 | 3.7 | 5.7 | 7.0 | 0.2 |
| 286 | 11-03 15:40 | 11-03 17:10 | 12 | 80.1 | 65.8 | 45.2 | 19.6 | 7.5 | 7.8 |
| 280 | 11-03 14:00 | 11-03 17:59 | 27 | 162.8 | 141.6 | 99.2 | 21.2 | 21.2 | 21.2 |
| 302 | 11-04 16:54 | 11-04 17:17 | 3 | 22.0 | 15.5 | 10.3 | 6.6 | 0.0 | 5.1 |
| 301 | 11-04 19:14 | 11-04 20:10 | 9 | 54.6 | 42.1 | 34.0 | 15.5 | 0.0 | 5.1 |
| 306 | 11-04 19:40 | 11-04 20:50 | 7 | 44.5 | 28.5 | 22.7 | 16.5 | 0.0 | 5.4 |

Minutes, UTC. Works 269 and 274 started before the switch and were merged after it (9 and 17 of their 27 and 21 calls
after it; 159.8 and 123.3 min): they are in neither group. The works from 301 on have no full CI on the draft (removed
from the flow on 2026-11-04, commit 8caa4e92): their full CI runs once, in `merge`, for 5.1 to 5.4 min.

**The baseline of 69.7 min.** #38's description gives it as the sum for works 238 to 268 that ran `ci` (25 works): 249
calls, 1340.8 min of `ci`, 262.4 of `ci_before`, 89.0 of `full_ci` and 50.2 of `merge`, "recomputed on 2026-11-02"; two of
the works were still going. The same 25 works, counted up to a moment (Q7, Q7b; a call counts when it was queued by the
moment and has ended by now):

| Counted up to (UTC) | works | `ci` calls | `ci` | `ci_before` | `full_ci` | `merge` | total | per work |
|---|---|---|---|---|---|---|---|---|
| 2026-11-03 04:33:33 (the 248th `ci` call) | 25 | 248 | 1340.6 | 257.6 | 89.0 | 50.2 | 1737.4 | 69.5 |
| 2026-11-03 04:37:22 (the 249th `ci` call) | 25 | 249 | 1345.0 | 262.4 | 89.0 | 50.2 | 1746.6 | 69.9 |
| 2026-11-03 04:39:01 (the 250th `ci` call) | 25 | 250 | 1345.2 | 262.4 | 89.0 | 50.2 | 1746.7 | 69.9 |
| 2026-11-03 06:00 | 25 | 287 | 1556.0 | 316.0 | 89.0 | 57.7 | 2018.7 | 80.7 |
| 2026-11-03 09:00 | 25 | 323 | 1727.3 | 327.5 | 96.7 | 64.6 | 2216.0 | 88.6 |
| to the cutoff (2026-11-04 21:18) | 25 | 326 | 1733.9 | 327.5 | 96.7 | 64.7 | 2222.8 | 88.9 |

The figure of 69.7 min is reproduced (measured). At the 249th `ci` call, 2026-11-03 04:37 UTC (2026-11-02 evening in
Pacific time, which fits "recomputed on 2026-11-02"), the number of calls is #38's 249, and `ci_before` 262.4, `full_ci`
89.0 and `merge` 50.2 min are #38's to the decimal. The `ci` figure is 1345.0 against #38's 1340.8, 4.2 min more, which
makes 69.9 min a work against 69.7. The 4.2 min is probably the 249th call itself, still running when #38 was
written and counted for only a part of its length (inferred; the description does not say how a running call was
counted). The baseline is thus the sum for works not finished, as the description says, at that moment; the same 25
works come to 80.7 min by 06:00 and 88.9 min by the cutoff. For finished works the baseline is 88.9 min (the same 25) or
93.7 min (PRE25), and half of it is 44.5 to 46.9 min, not 34.8. Against that, the median of POST (44.5 min) is at the
line and the mean (60.1 min) is above it.

**Calls against minutes a call.** Per work, `ci` took 72.4 min in PRE25 and 33.7 min in POST (-53%): 13.6 against 9.3
calls (x0.68) and 5.30 against 3.63 minutes a call (x0.68). The fall in the calls a work makes is not established: the
medians are 13 and 7, the intervals overlap, and one work made 27. The other parts per work: `ci_before` 14.7 to 12.9 min,
`full_ci` 3.9 to 7.1 min and `merge` 2.9 to 6.4 min: the full CI and the merge together cost a work 6.7 min in PRE25 and
13.5 min in POST. The full CI ran on the draft for the works up to 287 and, for 280 and 286, again inside `merge`; for
the works from 301 on it runs once, inside `merge`.

## 6. The full CI the first time, and `ci` with `ci_before`

**The first full CI green** (Q8). The first full CI of a work is the earliest of its rows of `full_ci` and `merge` that
hold a list of checks; green means that no check is red or cancelled. A work whose merge conflicted before any CI ran has
none. Before the switch 42 of 43 works were green the first time (97.7%; 95% Wilson interval 88% to 99.6%): 41 of 42 in
`full_ci` and 1 of 1 in `merge`. After it 10 of 12 (83.3%; 55% to 95%): 7 of 9 in `full_ci` and 3 of 3 in `merge`. The
works 238 to 268 give 22 of 23 (95.7%); #38 counted 24 of 25. The intervals overlap, so no change is told. Why the two
works were red was not read; the fast CI runs only the named tests, so a failure outside them shows first in the full
CI (inferred). At 12 works this cannot be read as more than a description.

**The two tools together** (Q3, Q4). Rounds that asked for both, the first row of each tool, in minutes:

| Rounds | n | `ci` | `ci_before` | the shorter | the round |
|---|---|---|---|---|---|
| ran one after the other (whole CI), all | 127 | 5.39 | 4.58 | 3.67 | 9.96 |
| the same, `ci` 10 s or more | 110 | 6.20 | 4.60 | 4.22 | 10.80 |
| the same, queued before 2026-11-03 00:00 UTC | 69 | 4.82 | 4.88 | 3.69 | 9.71 |
| side by side, all | 59 | 3.50 | 2.85 | 2.82 | 3.53 |
| side by side, `ci` 10 s or more | 58 | 3.56 | 2.89 | 2.87 | 3.59 |

The design's figures (118 rounds, `ci` 5.33 min, `ci_before` 5.15 min, at most 3.73 min saved) are not reproduced; the
nearest set is the first row (127 rounds, 5.39, 4.58, 3.67), a few days later. The 59 side-by-side rounds ran from
2026-11-03 11:00 UTC; the last one-after-the-other round was queued at 08:42 UTC, and the switch (10:52 UTC) lies
between (the same engine installation, inferred). In them `ci_before` adds 0.03 min (2 s) to the round, so the overlap saves
2.85 - 0.03 = 2.82 min a round, the whole of the shorter run: 100% of the most it can save, 76% of the design's 3.73 min,
which was computed on the longer whole-CI runs. The round fell from 9.96 min to 3.53 min.

Are the runs longer for being side by side? (Q4, Q4b; the intervals are of the median.)

| Calls | n | median | 95% interval | wait for a runner (test job, sample) |
|---|---|---|---|---|
| `ci` with a `ci_before` in the round | 58 | 218.4 s | 200.7 to 221.9 s | median 3 s, mean 2.6 s (14 jobs) |
| `ci` alone | 103 | 218.1 s | 202.9 to 219.5 s | median 3 s, mean 3.4 s (24 jobs) |
| `ci_before` with a `ci` in the round | 68 | 162.8 s | 158.6 to 178.1 s (ranks 25, 44) | median 3 s, mean 2.6 s (11 jobs) |
| `ci_before` alone | 11 | 158.8 s | 138.1 to 178.9 s | median 3 s, mean 3.6 s (11 jobs) |

Neither run is told longer: the intervals overlap in both pairs, and the wait for a runner is 3 s in all four groups. The
lint job's wait is the same (median 2.5 and 3 s). The group of `ci_before` alone is 11 calls, all there are, and its
interval is 40 s wide: a lengthening of up to about 20 s of `ci_before` is not excluded. GitHub's run times in the sample:
`ci` 201 s with a `ci_before` in the round (interval 160 to 214 s, 14 runs) and 192 s without (173 to 202 s, 24 runs);
`ci_before` 133 s with a `ci` (108 to 167 s) and 129 s without (107 to 155 s), 11 runs each. The medians are 9 s and 4 s
higher with the other tool, well inside the intervals.

A round, as the records show it (work 312, round 16, 2026-11-04 UTC): `ci` 21:00:24 to 21:04:20 (237 s), `ci_before`
21:00:26 to 21:03:44 (198 s); the runs were created at 21:00:28 and 21:00:29, and ended at 21:03:58 and 21:03:30. The
test job of `ci_before` waited 4 s for its runner.

## 7. What is worth changing

Per call and per work, with the 9.3 calls a work made in POST. Only the first row is a figure that was measured whole;
the others are arithmetic on measured segments.

| Change | Evidence | Saves a call | Saves a work | Known how |
|---|---|---|---|---|
| Keep the built page (`client/dist`) from a cache keyed by the hash of `client/`, and build it only when that changes | the page is 63 s, 29% of `ci` and 36% of `ci_before`; restoring the 85 MB node cache takes 0.5 to 1.6 s in the logs | about 55 to 60 s, of both tools | about 8.5 to 9.3 min, while `ci` is the longer of the pair (it is, by 60 s) | inferred; not tried |
| Look at the run every 5 s instead of 20 s | after-run time: floor 8 s, median 18 s, mean 19.7 s | about 7 to 8 s | about 1.2 min | inferred from the shape |
| Lint off the critical path | lint ended later in 35% of the calls that ran it, by 42 s | done on 2026-11-04 (5.7% of the mean call) | | measured |
| Main's build state | restore 24 s, saves 27 s; over 38 calls the restore is 3 s (mean) and 10 s (median) faster, on 11-04 4.8 s slower (5 against 2 calls) | undecided | undecided | observed, not paired; needs paired runs |
| Setting the job up | 26 s a call; the split between toolchain, nextest, rust-cache and node is not measured | not known | not known | measure first |
| The tests themselves | 3 to 7 s a call | nothing to gain | | measured |

To reach 34.8 min as the mean of the union count (POST: 49.3 min) a work must lose 14.5 min: with 9.3 calls that is 94 s
off each call of 216 s (to about 122 s). Or, at the present time per call, fewer calls: if the whole wait scales with
the calls, 9.3 x 34.8 / 49.3 = 6.6 calls instead of 9.3; if only the 35.8 min of `ci` and `ci_before` (49.3 - 13.5)
scale and the 13.5 min of the full CI and the merge stay as they are, (34.8 - 13.5) / (35.8 / 9.3) = 5.5 calls
(inferred; both readings take the means of POST). The page and the
polling together would take about 66 s (inferred) and leave 28 s, the size of the job set-up. On the median (37.5 min,
for the work with 7 calls) the page alone would suffice (about 31 min after). So the median can plausibly get below 34.8
min with one change; the mean cannot without the set-up or fewer calls, and the target was set as half of a snapshot
that was low.

The calls a work makes matter as much as the time of each: 9.3 calls, of which 37% had a red check. Fewer round trips
(a call for each fix) is the engineer's and the flow's, not the CI's; this report does not measure it.

## 8. What was not measured, and the limits

- The groups are small where it matters: 7 works after the switch, 12 first full CIs, 11 `ci_before` calls alone, 7
  restores not found. Every figure from them is a description of those, not a test.
- The sampled runs are from 2026-11-03 11:00 to 2026-11-04 21:19 UTC, most of them from the first day; the 7 from the
  second day are the ones after the lint became optional.
- The parts of the job set-up, the full CI that runs inside `merge`, and the effect of the runners' load by hour were not
  measured. GitHub's times have a resolution of 1 s.
- Nothing here was tried: the savings in section 7 are arithmetic on the segments, and any change made from them must be
  measured before and after with paired runs, as the project's measurement rules require.
- No decision in force is departed from; no engine, CI or flow file was changed.

## 9. To reproduce

- `ci-speed-measured-queries.sql` Q1 to Q9 (and Q1b, Q7b) give sections 3, 5 and 6; Q10 gives the sample of runs. All
  but Q1b and Q10 stop at the cutoff of 2026-11-04 21:18:53 UTC, so they give the numbers of the report whenever they run.
- The runs of `fast.yml`, completed (GitHub's run time of section 3):
  `gh run list --workflow fast.yml --limit 300 --json headBranch,startedAt,updatedAt,status`, with the runs on
  `wf-agents/...` branches as `ci` and those on `wf-agents-before/...` as `ci_before`.
- The times of the jobs of a run (section 4):
  `gh api repos/<repo>/actions/runs/<run>/jobs`, whose `jobs[].created_at`, `started_at`, `completed_at` and
  `steps[].started_at`, `completed_at` are summed as the header of `ci-speed-measured.sql` says. The result is kept in
  that file, since GitHub keeps the records of a run for 90 days only.
- The logs (section 4): `gh run view <run> --log --job <job>`; the lines `Finished`, `Summary`, `added ... packages`,
  `built in` and `Cache Size` are the ones read.
