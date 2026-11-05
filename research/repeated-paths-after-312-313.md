# Paths that run again and again, after #312 and #313, measured on 2026-11-05

Item #315. It repeats the measurement of `research/repeated-paths.md` (item #248) on the code with #312 (indexes for the
reads that scanned a whole table, `3cc9c10c`) and #313 (a work that reached the end of its graph is ended as `done`,
`40b70107`) in it, and ranks what is left. It changes no engine, interface, page or database file.
Whom it serves: the architect, who opens the fix items from the ranking in section 4 and orders them, and the owner,
who wants to know how many works the system can carry before it is slow. It is kept until the fixes of section 4 are in
and the program is run again.

Files, all in `research/`:

- `repeated-paths-after-312-313.md` (this report);
- `repeated-paths/repeated_paths.py`: the program. Since #248 it builds the done works as #313 leaves them
  (`ended = 'done'`) and takes the next transaction number with #312's statement; its `summarize` mode now reads the
  three sides of a `measure` run apart and prints the change from the base to the candidate;
- `repeated-paths/results-after-312-313.txt`: the summary of this run as the program printed it: the growth tables of the
  base and of the candidate, and the change from one to the other at each scale, for every path and view;
- `repeated-paths/raw-after-312-313.tsv`: the raw lines of the 15 processes (`PROCESS`, `SQLITE`, `DBSIZE`, `PATHTIME`,
  `DONE`), from which `summarize` makes the file above. The table-count lines (`TABLE`, `CHECK`) are not kept; they were
  all `ok` (15 processes ended `DONE ok`);
- `repeated-paths/plans-after-312-313.txt`: the query plan of every path as the last candidate process and the last base
  process of the run gave it. The summary's own `PLAN` lines are of whichever process ran last (a base process), so they
  show the base's plans; this file has both sides;
- `repeated-paths.md` and `repeated-paths/results-2026-11-05.txt`: the report of item #248 and its run, the "#248" column
  below.

To run it again (the `measure` tool, workflow `measure.yml`, five pairs; each of the 15 processes took 100 to 121 s, the
whole run about 30 minutes; the `base` is a commit before the change to compare with, here the commit of #248):

```
command=python3 -I research/repeated-paths/repeated_paths.py run --scales 1,1L10,10 | tee -a "$RUNNER_TEMP/rp-all.log"; python3 -I "$RUNNER_TEMP/candidate/research/repeated-paths/repeated_paths.py" summarize "$RUNNER_TEMP/rp-all.log"
base=b101aa9bcd79afe64b123a07b2842aef46df2d1c
pairs=5
```

Each claim says how it is known: (measured) by this run, (read) from the source or the records, (inferred) from those,
not tried.

## 1. The answers

| Question | Answer |
|---|---|
| Did #312 do what it was for | For the paths it named, yes (measured). The next transaction number, inside the write lock, is 274.8 ms at ten times the data before and 0.006 ms after, and no longer grows with it (E2: 0.995 to 1.009 noise, ratio after/before below 0.001). Reading one conversation at ten times the data went from 750 and 757 ms to 24.5 and 30.2 ms; `/conversations` from 1,512 to 158 ms; `/flows` from 71.5 to 0.77 ms; `/observations` from 154 to 45.5 ms; the rounds of a matter from 154 to 86 ms; an engineer's session from 114 to 0.35 ms (all measured, section 3). The paths whose cost is not a missing index (`/works`, `/system`, the architect's long work, `work_counters`, `architect_time`, `resumed_at`, the views read whole) did not change, as they should not. |
| Did #312 break anything | Two reads got slower because of its indexes (measured). `GET /items` (`api.items_extra`): 7.9 ms before and 17.8 ms after at the field's size, 106.8 ms before and 5,596 ms after at ten times (50.6 to 54.7 times, noise 0.990 to 1.003): the index `rounds_by_cell` makes the view `work_routes` look up every round of the cell `D1` for each work, where it used to look up the work's own rounds. And `own_rounds` of the architect's long session: 25.2 to 35.1 ms now, 160.7 to 356 ms at ten times the long work, because the index `rounds_by_session` finds the rows in the order of their row id and the statement sorts them by `work, n`. The first is a quadratic path that was linear; the architect has taken it as item #316. Section 3, rows I5 and R2. |
| Did #313 do what it was for | Yes, for the passes it named (measured, on a database built as the migration leaves it). The loop's three passes over the works not ended load 54 works instead of 304 (540 instead of 3,040 at ten times): 17.3, 17.8 and 18.3 ms before, 1.4, 1.6 and 2.2 ms after (ten times: 175, 178, 185 before, 17.9, 21.7, 27.1 after). They still grow with the number of works not ended (ratio of the time at ten times to the time at the field's size 12.3 to 13.4, the rows being ten times as many and the rest the cache effect described in #248), because the recipe makes every kind of work ten times as many; whether the owner's open and paused works grow like that is not known. |
| Is #313 in effect on the engine's own database | No (read from the records, `query` on 2026-11-05): of 374 works, 324 have `ended` empty, none has `ended = 'done'`, and 311 are not paused: the migration has not been applied to the database of the engine that is running, which is the engine built before #313. The numbers for #313 are those of a synthetic database built as the migration leaves it. |
| What costs most now | By the ranking of section 4: the page's refresh (one flush of its eight reads: 9.4 s at ten times, of which `/items` 5.6 s, `/system` 3.1 s, `/works` 0.56 s), then `architect_time`, the architect's long session and long work, `resumed_at`, and the rest. The loop's passes (E1) fell from the largest cost that is always there to 85 to 128 ms per turn at ten times (1,100 to 1,700 before) and a turn with `reload_config` is 53 to 57 ms now, 132 to 175 ms at ten times (167 to 220 before and 1.1 to 1.7 s). |
| Which paths must be fixed through `flow/flow.json` | None of the ranked paths needs a change to `flow/flow.json` itself (inferred from the fixes section 4 sketches: they are in the code, the schema, the interface and the page). A fix of four of them would change a view the flow reads (`work_counters`, `side_reports`, and `ready_items` and `open_items` if their definition of done changes; `work_routes` if the view and not the index is changed): `flow/flow.json` names those views (read: `grep` of the file), and a change to them needs the owner's approval at the merge (the project guide). They are marked in section 4. |
| What was not measured | The same as in the report of #248 (the page's time, the Rust code, `keep_warm`'s session reads E6, the hidden tables of R10), and the rates of section 4, which are those of the report of #248 and were not measured again. |

## 2. How the numbers were taken

- **The run.** One run of `measure.yml` (<https://github.com/Ming-Research/wf-agents/actions/runs/42111728015>), five
  pairs, each pair a process of the `base`, of its `twin` and of the `candidate`, in a rotating order, on one
  `ubuntu-24.04` runner of 4 processors, SQLite 3.45.1 of Python. The `base` is `b101aa9b` (#248 merged), with its own
  copy of the program and its own `db/schema.sql`; the `candidate` is the branch of this item (the program changed in the
  four ways of the next point, on `main` with #312 and #313). Each side builds its own databases at the three scales
  (the field's size, `1`; the field's size with the architect's longest work ten times as long, `1L10`; ten times the
  field, `10`) and times every path 5 times (fewer for slow ones) after a warm-up, keeping the median.
- **What changed in the program** (all `repeated_paths.py`): a work in the state "done" has `ended = 'done'`
  (`db::ENDED_DONE`, read from `crates/db/src/lib.rs` and the migration `202611060100`); `engine.write.next_tx` is
  `SELECT coalesce((SELECT max(tx) FROM changes), 0) + 1` as `Db::write` has it now (`NEXT_TX`); the summary reads the
  three sides apart; the command calls the candidate's copy of the summary (the base's copy is older). Every other
  statement was compared with the diff of #312 and #313 and kept: the SQL texts of `own_rounds`, `rounds_of_matter`,
  `words_given`, `asked`, `flows` and the views are the same, the new indexes and views come from the candidate's schema.
  The recipe (the figures of the field on 2026-11-05) was not changed, so the base here is the same database as in #248.
- **The comparison.** For a path and a scale the table gives the median of the base's processes and of the candidate's,
  the ratio candidate/base of each pair with its 95% interval (Student's t on the logarithms, five pairs), and the noise,
  the twin's time over the base's with its interval. The rule is the one of #248 (A3 of that item): an interval wholly
  below 1 is "faster", wholly above 1 is "slower", across 1 is "no change detected"; a path whose noise interval excludes
  1 is "unresolved". Some paths with a tight interval are "slower" by 1 to 3% (`api.works`, `api.system.*`, `work_counters`,
  `architect_time`) without any change in their statement or their plan: the two sides do not run on the same file, and a
  difference of that size is not read as an effect of the change (inferred from the unchanged statements and plans); the
  paths that did change moved by 2 to 50 times.
- **The "#248" column** is the run of the report of item #248 (three pairs, commit `95ac342f`), a different day and runner
  state. It differs from the base of this run by up to 35% on the same code (`engine.turn.active_pass` 26.0 against 17.3 ms,
  `engine.write.next_tx` 33.1 against 27.3 ms), which is why the change is read candidate/base in this run and not
  against the old file. The old paths `api.architect.matters`, `view.architect_queue` and `api.items_extra`, that
  the old report left unresolved for noise, have noise intervals across 1 here at the field's size and at ten times
  (`api.items_extra` at the field's size is 0.979 to 0.999, narrowly excluding 1, and its change, 2.2 times, is far larger).
- **The synthetic data** has the limits written in section 2 of the report of #248, and one more that matters for #313:
  scale 10 makes every kind of work ten times as many, so the works not ended also number ten times as many (540 against 54).
- **Rates** (how many times in 10 s) are those of the report of #248, measured then from the records (writes, rounds) or
  read from the source (a turn, a flush), not measured again.

## 3. The paths, at the three scales

Milliseconds. "#248": the report of item #248. "base": the code before #312 and #313, in this run. "after": the candidate.
"after/base": the 95% interval of the ratio over the five pairs, then the verdict by the rule of section 2. Rows are the
rows of the ranking table of `research/repeated-paths.md` section 3 (the E, R, I, P numbers are its), then the
paths those rows name. The rows each path read are in the table "Rows read" after the tables of milliseconds (base, then
after, where they differ).

### Engine, every turn of the loop

| Path | Scale | #248 | base | after | after/base | verdict |
|---|---|---|---|---|---|---|
| E1 `engine.turn.active_pass` (rows 304, then 54) | 1 | 26.0 | 17.3 | 1.41 | 0.079 to 0.083 | unresolved (noise 1.001 to 1.010; 12 times) |
| | 1L10 | 25.9 | 17.2 | 1.39 | 0.078 to 0.083 | faster |
| | 10 (rows 3,040, then 540) | 262.0 | 175.0 | 17.9 | 0.100 to 0.106 | faster |
| E1 `engine.turn.slots_pass` | 1 | 26.5 | 17.8 | 1.63 | 0.090 to 0.094 | faster |
| | 1L10 | 26.4 | 17.8 | 1.65 | 0.090 to 0.094 | faster |
| | 10 | 268.2 | 177.9 | 21.7 | 0.118 to 0.124 | faster |
| E1 `engine.turn.keep_warm_scan` (rows 318, then 68) | 1 | 27.3 | 18.3 | 2.17 | 0.117 to 0.122 | faster |
| | 1L10 | 27.3 | 18.3 | 2.15 | 0.116 to 0.121 | faster |
| | 10 (rows 3,180, then 680) | 276.9 | 185.0 | 27.1 | 0.145 to 0.149 | faster |
| E2 `engine.write.next_tx` | 1 | 33.1 | 27.3 | 0.006 | below 0.001 | faster |
| | 1L10 | 33.1 | 27.5 | 0.006 | below 0.001 | faster |
| | 10 | 333.9 | 274.8 | 0.006 | below 0.001 | faster |
| E3 `engine.turn.reload_config` (timed at the field's size only: its cost is the files of the configuration, not the data) | 1 | 30.8 | 23.6 | 23.4 | 0.975 to 1.021 | no change detected |
| E4 `engine.turn.reminder` | 1 | 0.356 | 0.257 | 0.046 | 0.174 to 0.183 | faster |
| | 1L10 | 0.608 | 0.347 | 0.172 | 0.269 to 0.677 | unresolved (noise 0.979 to 0.991) |
| | 10 | 88.3 | 66.9 | 1.43 | 0.020 to 0.022 | faster |
| E5 `engine.turn.ready_items` | 1 | 5.44 | 4.39 | 4.10 | 0.898 to 0.995 | faster |
| | 1L10 | 6.17 | 5.07 | 4.75 | 0.905 to 0.990 | faster |
| | 10 | 84.2 | 69.6 | 65.6 | 0.931 to 0.958 | faster |
| E5 `engine.turn.slots_free_history` | 1 | 0.150 | 0.118 | 0.106 | 0.764 to 1.043 | no change detected |
| | 1L10 | 0.144 | 0.105 | 0.103 | 0.848 to 1.032 | no change detected |
| | 10 | 5.29 | 4.50 | 4.58 | 0.928 to 1.081 | no change detected |
| E6 `keep_warm` session reads | all | not measured | not measured | not measured | | the number of calls per turn is not known (Article 4: record it) |

Per turn, adding the parts by the number of calls the source makes (2 `active`, 1 to 3 `slots_by_machine`, 1 `keep_warm`
scan, 2 `reload_config`; read in #248): at the field's size 167 to 220 ms in #248, 53 to 57 ms after (`2 x 1.41 +
(1.63 to 4.9) + 2.17 + 2 x 23.4`); at ten times 1.1 to 1.7 s in #248, 132 to 175 ms after (inferred, sum of measured parts).
`reload_config` (2 x 23.4 ms) is now 27% to 36% of a turn at ten times and 83% to 88% at the field's size.

### Engine, per write, step and round

| Path | Scale | #248 | base | after | after/base | verdict |
|---|---|---|---|---|---|---|
| R1 `engine.round.rounds.arch` (the architect's long work read whole; 3,330 rounds, 33,300 at 1L10) | 1 | 27.2 | 24.8 | 24.9 | 0.960 to 1.040 | no change detected |
| | 1L10 | 294.5 | 262.4 | 266.2 | 1.004 to 1.024 | slower (2%) |
| | 10 | 27.8 | 25.4 | 25.8 | 0.969 to 1.035 | no change detected |
| R1b `engine.step.open_round.arch` | 1 | 3.32 | 2.77 | 2.52 | 0.847 to 1.032 | no change detected |
| | 1L10 | 34.0 | 29.3 | 29.1 | 0.990 to 1.000 | no change detected |
| | 10 | 3.17 | 2.83 | 2.73 | 0.922 to 1.087 | no change detected |
| R2 `engine.round.own_rounds.eng` (a session of an engineer) | 1 | 13.9 | 11.6 | 0.251 | 0.021 to 0.022 | faster |
| | 1L10 | 40.4 | 33.5 | 0.348 | 0.010 to 0.011 | faster |
| | 10 | 137.3 | 114.4 | 0.346 | 0.003 | faster |
| R2 `engine.round.own_rounds.arch` (the architect's long session; rows 1,689, 14,838 at 1L10) | 1 | 28.2 | 25.2 | 35.1 | 1.352 to 1.411 | slower |
| | 1L10 | 178.5 | 160.7 | 356.2 | 2.194 to 2.242 | slower |
| | 10 | 152.2 | 127.4 | 35.5 | 0.270 to 0.281 | faster |
| R3 `engine.step.resumed_at` | 1 | 4.08 | 3.58 | 3.52 | 0.944 to 1.001 | no change detected |
| | 1L10 | 4.04 | 3.54 | 3.53 | 0.935 to 1.019 | no change detected |
| | 10 | 41.2 | 36.9 | 36.6 | 0.953 to 1.007 | no change detected |
| R4 view `work_counters` read whole | 1 | 39.1 | 33.9 | 34.4 | 1.005 to 1.026 | slower (1.6%, see section 2) |
| | 1L10 | 105.5 | 89.9 | 90.9 | 1.005 to 1.019 | slower (1.2%) |
| | 10 | 395.5 | 337.4 | 337.6 | 0.996 to 1.004 | unresolved (noise 1.001 to 1.009) |
| R4 `engine.round.counters_all` | 1 | 40.5 | 34.6 | 35.3 | 1.006 to 1.026 | slower (1.6%) |
| | 1L10 | 108.1 | 90.7 | 92.0 | 1.006 to 1.022 | slower (1.4%) |
| | 10 | 408.4 | 348.2 | 348.7 | 0.997 to 1.007 | no change detected |
| R5 `engine.round.rounds_of_matter` | 1 | 17.1 | 14.9 | 8.09 | 0.535 to 0.547 | faster |
| | 1L10 | 75.8 | 67.8 | 56.8 | 0.804 to 0.865 | faster |
| | 10 | 178.4 | 154.2 | 85.9 | 0.549 to 0.561 | faster |
| R6 `engine.round.architect_time` | 1 | 21.5 | 18.8 | 19.1 | 1.003 to 1.022 | slower (1.3%, see section 2) |
| | 1L10 | 103.5 | 86.2 | 88.7 | 1.016 to 1.034 | slower (2.5%) |
| | 10 | 204.3 | 175.1 | 175.5 | 0.994 to 1.011 | unresolved (noise 1.001 to 1.005) |
| R7 `engine.round.interjections_words` | 1 / 10 | 0.031 / 0.64 | 0.022 / 0.479 | 0.007 / 0.008 | 0.302 to 0.341 / 0.017 | faster |
| R7 `engine.round.asked_matters` | 1 / 10 | 0.084 / 4.1 | 0.066 / 3.44 | 0.006 / 0.006 | 0.088 to 0.092 / 0.002 | faster |
| R8 `engine.rare.merges_of_commit` | 1 | 75.6 | 69.6 | 69.8 | 0.997 to 1.011 | no change detected |
| | 1L10 | 198.2 | 178.2 | 179.0 | 1.000 to 1.014 | no change detected |
| | 10 | 762.6 | 701.8 | 697.3 | 0.989 to 1.005 | no change detected |
| R9 `engine.round.ci_results`, `fact_basis` | 1 / 10 | 0.36, 1.2 / 0.47, 1.5 | 0.306, 1.12 / 0.425, 1.39 | 0.305, 1.15 / 0.423, 1.41 | 0.952 to 1.078, 0.995 to 1.037 / 0.958 to 1.028, 1.002 to 1.023 | no change detected (`fact_basis` at ten times 1% slower) |
| R10 `round_measures` read for each round with a guard | all | not measured | not measured | not measured | | the table cannot be read by `query`; the program builds it empty |

### Interface

| Path | Scale | #248 | base | after | after/base | verdict |
|---|---|---|---|---|---|---|
| I1 `api.system.works_of` | 1 | 66.6 | 58.4 | 59.1 | 1.008 to 1.018 | slower (1.3%) |
| | 1L10 | 228.6 | 195.2 | 198.6 | 1.014 to 1.023 | slower (1.8%) |
| | 10 | 678.1 | 588.1 | 588.2 | 0.987 to 1.008 | no change detected |
| I1 `api.system.rounds_since` | 1 | 109.0 | 102.0 | 103.3 | 1.001 to 1.019 | slower (1.0%) |
| | 1L10 | 179.8 | 159.8 | 163.1 | 1.009 to 1.028 | slower (1.9%) |
| | 10 | 623.8 | 558.3 | 557.7 | 0.996 to 1.004 | no change detected |
| I1 `api.system.counted` | 1 | 67.3 | 60.6 | 61.4 | 1.002 to 1.022 | slower (1.2%) |
| | 1L10 | 198.4 | 177.0 | 178.9 | 0.996 to 1.023 | no change detected |
| | 10 | 691.4 | 620.1 | 619.9 | 0.981 to 1.014 | no change detected |
| I1 `api.system.work_changes` | 1 | 63.9 | 58.3 | 58.3 | 0.958 to 1.008 | no change detected |
| | 1L10 | 64.3 | 59.3 | 58.8 | 0.975 to 1.003 | no change detected |
| | 10 | 798.9 | 739.7 | 728.6 | 0.974 to 0.992 | faster (1.5%) |
| I1 `api.system.item_events` | 1 | 44.8 | 40.0 | 40.2 | 0.983 to 1.034 | no change detected |
| | 1L10 | 45.0 | 40.2 | 40.0 | 0.976 to 1.028 | no change detected |
| | 10 | 664.3 | 589.3 | 586.5 | 0.989 to 1.010 | no change detected |
| I2 `api.works` (rows 7,727; 77,259 at ten times) | 1 | 58.5 | 52.9 | 53.9 | 1.000 to 1.024 | slower (1.2%) |
| | 1L10 | 58.2 | 53.0 | 53.5 | 0.994 to 1.012 | no change detected |
| | 10 | 619.6 | 557.5 | 562.4 | 0.998 to 1.010 | no change detected |
| I2 `api.conversations` | 1 | 18.6 | 16.2 | 11.7 | 0.714 to 0.733 | faster |
| | 1L10 | 19.1 | 16.7 | 12.1 | 0.714 to 0.732 | faster |
| | 10 | 1,972.4 | 1,511.7 | 158.1 | 0.103 to 0.105 | faster |
| I3 `api.conversation.work` | 1 | 5.67 | 4.61 | 1.87 | 0.397 to 0.416 | faster |
| | 1L10 | 5.89 | 4.78 | 2.12 | 0.427 to 0.445 | faster |
| | 10 | 1,014.7 | 750.1 | 24.5 | 0.032 to 0.033 | faster |
| I3 `api.conversation.architect` | 1 | 5.84 | 4.74 | 2.12 | 0.426 to 0.457 | faster |
| | 1L10 | 6.01 | 4.87 | 2.21 | 0.437 to 0.463 | faster |
| | 10 | 1,018.4 | 757.2 | 30.2 | 0.040 to 0.041 | faster |
| I3 view `conversation_entries` | 1 | 6.61 | 5.46 | 2.72 | 0.469 to 0.518 | faster |
| | 1L10 | 6.74 | 5.61 | 2.71 | 0.456 to 0.510 | faster |
| | 10 | 1,040.7 | 774.3 | 55.0 | 0.070 to 0.072 | faster |
| I4 `api.work.arch` (the page of the architect's long work, 29.8 MB at 1L10 ten times that) | 1 | 73.8 | 71.3 | 71.3 | 0.986 to 1.007 | no change detected |
| | 1L10 | 767.5 | 720.7 | 727.5 | 0.998 to 1.012 | unresolved (noise 1.001 to 1.014) |
| | 10 | 72.5 | 70.7 | 70.8 | 0.978 to 1.013 | no change detected |
| I5 `api.items_extra` (`/items`; rows 713, 7,174 at ten times) | 1 | 9.29 | 7.91 | 17.8 | 2.211 to 2.278 | unresolved (noise 0.979 to 0.999; 2.2 times) |
| | 1L10 | 9.44 | 8.48 | 20.4 | 2.344 to 2.458 | slower |
| | 10 | 124.3 | 106.8 | 5,596.3 | 50.581 to 54.670 | slower |
| I5 `api.architect.rounds_window` (`/architect/rounds`) | 1 | 22.8 | 20.0 | 20.0 | 0.985 to 1.018 | no change detected |
| | 1L10 | 116.3 | 98.6 | 99.1 | 1.003 to 1.016 | slower (0.9%) |
| | 10 | 231.6 | 199.9 | 198.9 | 0.991 to 1.004 | no change detected |
| I5 `api.observations` | 1 | 17.1 | 14.5 | 3.25 | 0.216 to 0.241 | faster |
| | 1L10 | 42.7 | 36.2 | 3.16 | 0.085 to 0.090 | faster |
| | 10 | 180.7 | 154.1 | 45.5 | 0.290 to 0.303 | faster |
| I5 `api.flows` | 1 | 0.975 | 0.786 | 0.072 | 0.079 to 0.101 | faster |
| | 1L10 | 0.972 | 0.770 | 0.077 | 0.094 to 0.106 | faster |
| | 10 | 92.0 | 71.5 | 0.770 | 0.010 to 0.011 | faster |
| I5 `api.architect.notebook` | 1 | 3.05 | 2.61 | 2.71 | 0.938 to 1.141 | no change detected |
| | 1L10 | 3.31 | 3.03 | 3.00 | 0.902 to 1.087 | no change detected |
| | 10 | 52.3 | 45.3 | 45.7 | 0.988 to 1.019 | no change detected |
| I5 `api.item.level_changes` | 1 | 7.13 | 6.46 | 6.39 | 0.954 to 0.998 | faster (1.0%) |
| | 1L10 | 7.08 | 6.48 | 6.44 | 0.939 to 1.014 | no change detected |
| | 10 | 65.6 | 58.8 | 58.6 | 0.959 to 1.006 | no change detected |
| I6 `api.events.row_lookups` (200 lookups) | 1 | 4.10 | 2.26 | 2.27 | 0.969 to 1.036 | no change detected |
| | 1L10 | 4.11 | 2.27 | 2.28 | 0.983 to 1.030 | no change detected |
| | 10 | 4.2 | 2.30 | 2.30 | 0.977 to 1.026 | no change detected |
| P1 one flush of the page: the eight reads `api.works`, `api.items_extra`, `api.conversations` and the five of `/system` (sums of medians; no interval) | 1 | 438 | 398.5 | 405.5 | | the sum has no interval; the part that changed is `api.items_extra` |
| | 1L10 | | 709.9 | 726.0 | | |
| | 10 | 6,182 | 5,287.8 | 9,406.8 | | the 4.1 s more is `api.items_extra` |
| P2, P3 the page's parse and redraw | all | not measured | not measured | not measured | | CI has no browser for it |

### Views read whole (by an agent's `query`)

| View | Scale | #248 | base | after | after/base | verdict |
|---|---|---|---|---|---|---|
| `side_reports` | 1 | 71.8 | 66.2 | 67.0 | 0.997 to 1.018 | no change detected |
| | 1L10 | 187.2 | 169.0 | 170.6 | 0.999 to 1.019 | no change detected |
| | 10 | 727.9 | 668.5 | 670.5 | 0.987 to 1.008 | no change detected |
| `not_applicable_answers` | 1 | 75.3 | 69.2 | 70.1 | 0.999 to 1.017 | no change detected |
| | 1L10 | 197.2 | 177.9 | 179.9 | 0.999 to 1.020 | no change detected |
| | 10 | 760.4 | 700.0 | 698.9 | 0.992 to 1.002 | no change detected |

The other views changed only where an index serves them: `item_research` (116.9 to 32.0 ms at ten times), `work_followups`
(191.7 to 97.3), `observations_all` and `observations_open` (32.5 and 32.8 ms to 0.11 at 1L10), `item_verifications`,
`known_red_counts`, `policy_current` and `policy_draft` (30 ms and above to below 0.02) and `conversation_shown` (`results-after-312-313.txt`,
every view, each scale). The views `ready_items` (0.812 to 0.882 of the base at the field's size) and `open_items` (0.910 to 0.966) are 3% to 19%
faster, probably because fewer works have `ended` empty for their `running` part to check (inferred, not isolated).

### Rows read

The rows the path returned (measured; "base → after" where the two sides differ; for a path that counts, `count(*)` or
a point lookup, 1 or 0). The fields of the tables above come from `results-after-312-313.txt`, columns "rows base" and
"rows candidate".

| Path | Scale 1 | Scale 1L10 | Scale 10 |
|---|---|---|---|
| E1 `engine.turn.active_pass`, `engine.turn.slots_pass` | 304 → 54 | 304 → 54 | 3,040 → 540 |
| E1 `engine.turn.keep_warm_scan` | 318 → 68 | 318 → 68 | 3,180 → 680 |
| E2 `engine.write.next_tx`, R3 `engine.step.resumed_at`, R6 `engine.round.architect_time`, R9 `fact_basis` | 1 | 1 | 1 |
| E3 `engine.turn.reload_config` (field's size only) | 18 files | | |
| E4 `engine.turn.reminder` | 2 → 1 | 2 | 2 |
| E5 `engine.turn.ready_items` | 48 | 50 | 427 |
| E5 `engine.turn.slots_free_history` | 1 | 1 | 1 |
| R1 `engine.round.rounds.arch`, I4 `api.work.arch` | 3,330 | 33,300 | 3,330 |
| R1b `engine.step.open_round.arch`, R5 `rounds_of_matter`, R8 `merges_of_commit` | 0 | 0 | 0 |
| R2 `own_rounds.eng` | 29 | 39 | 35 |
| R2 `own_rounds.arch` | 1,689 | 14,838 | 1,701 |
| R4 `engine.round.counters_all` | 838 | 832 | 8,419 |
| R4 view `work_counters`, I3 view `conversation_entries`, `side_reports`, `not_applicable_answers` | 1 (a count) | 1 | 1 |
| R7 `interjections_words` / `asked_matters` | 1 / 0 | 1 / 0 | 1 / 0 |
| R9 `ci_results` | 45 | 49 | 62 |
| I1 `api.system.works_of` | 363 | 363 | 3,630 |
| I1 `api.system.rounds_since` | 14,845 | 13,289 | 41,247 |
| I1 `api.system.counted` | 14,284 | 43,884 | 142,814 |
| I1 `api.system.work_changes` | 15,774 | 15,774 | 157,740 |
| I1 `api.system.item_events` | 311 | 311 | 3,110 |
| I2 `api.works` | 7,727 | 7,726 | 77,259 |
| I2 `api.conversations` | 1,096 | 1,126 | 11,010 |
| I3 `api.conversation.work` | 9 | 18 | 9 |
| I3 `api.conversation.architect` | 129 | 129 | 1,320 |
| I5 `api.items_extra` | 713 | 722 | 7,174 |
| I5 `api.architect.rounds_window` | 100 | 100 | 100 |
| I5 `api.observations` | 280 | 280 | 2,800 |
| I5 `api.flows` | 56 | 56 | 560 |
| I5 `api.architect.notebook` | 410 | 410 | 4,100 |
| I5 `api.item.level_changes` | 0 | 2 | 0 |
| I6 `api.events.row_lookups` | 200 | 200 | 200 |
| P1 the flush | the sum of the rows of its eight reads (`api.works`, `api.items_extra`, `api.conversations`, the five of `/system`) | | |

## 4. What is left, by weight

Weight is the one number of the report of #248: the database time per 10 s at ten times the data, the time of one
occurrence at scale 10 (scale 1L10 for the architect's long work, which grows with age and not with the number of works)
times the occurrences in 10 s. The occurrences are those of #248, not measured again, except in the row R2, whose
occurrences and scale were chosen again (below): a turn of the loop once in 10 s
(read: `crates/wf-agents/src/main.rs:1046`); a flush of an open page 3.3 times in 10 s while events come (read:
`client/src/live.ts`); a write 87 times in the busiest 10 s (measured then, from `changes`); an agent round 13 times in
the busiest 10 s, 22% of them the architect's longest work's, so 2.9 (measured then, from `rounds`). "Flow" says whether
the fix changes `flow/flow.json` itself, or a view the flow reads, which needs the owner's approval at the merge (the
project guide). "Item" names an item that covers it.

| Rank | Path | One occurrence at 10x (ms) | Occurrences in 10 s | ms in 10 s at 10x | In #248 (rank, ms) | Rate known by | Flow |
|---|---|---|---|---|---|---|---|
| 1 | P1 one flush of the page (the eight reads) | 9,407 | 3.3 | 31,043 | (2) 20,400 | read | no |
| 2 | I5 `/items` (`api.items_extra`; part of 1) | 5,596 | 3.3 | 18,467 | 124 ms, not ranked | read | no for an index `rounds(cell, work)`; yes if `work_routes` is rewritten. Item #316 |
| 3 | I1 `/system`, five reads (part of 1) | 3,082 | 3.3 | 10,171 | (3) 11,405 | read | no |
| 4 | I2 `/works` and `/conversations` (part of 1) | 720 | 3.3 | 2,376 | (4) 8,554 | read | no |
| 5 | R6 `architect_time` | 175.5 | 13 at the busiest | 2,281 | (6) 2,652 | inferred (one per step of a waiting work) | no |
| 6 | R2 `own_rounds`, the architect's long session at 1L10 | 356.2 | 2.9 | 1,033 | (7) 1,781 to 1,976 | inferred (one per agent round); not like the #248 row: #248 took all 13 rounds of the busiest 10 s at scale 10 (137 to 152 ms); this row takes the architect's long session only (22% of 13, so 2.9) at 1L10, since that session grows with its age. At the #248 basis (scale 10, 13 rounds) the same path is 35.5 x 13 = 461 for the architect's session and 0.35 x 13 = 4.5 for an engineer's, so the fall from 1,781 to 1,976 is #312's index for the engineer's sessions, and for the architect's long session #312 is slower (160.7 to 356 ms at 1L10) | no |
| 7 | R3 `resumed_at` (twice per round) | 73.2 | 13 | 952 | (9) 1,071 | inferred | no |
| 8 | R1 the architect's long work read whole, at 1L10 | 266.2 | 2.9 | 772 | (10) 854 | inferred | no (a change to what a cell is given would be a change of its input: the owner's approval) |
| 9 | I5 `/architect/rounds` | 198.9 | 3.3 while open | 656 | (11) 764 | read | no |
| 10 | R5 `rounds_of_matter` | 85.9 | 2.9 | 249 | (12) 517 | inferred | no |
| 11 | E1 the passes over the works not ended | 85 to 128 | 1 per turn | 85 to 128 | (8) 1,100 to 1,700 | read (counts), measured (parts) | no; the definition of "done" in `ready_items` and `open_items` still reads `ended IS NULL OR ended = 'done'`, and a change to it is a change to views the flow reads |
| 12 | E5 `ready_items`, `slots_free_history` | 70.2 | 1 per turn | 70 | (13) 90 | read | no; a change to `ready_items` is a view the flow reads |
| 13 | E3 `reload_config` (twice) | 46.9 | 1 per turn | 47 | (15) 62 | read | no |
| 14 | R4 `work_counters` read whole | 337.6 | 0.06 at the busiest | 20 | (16) 25 | inferred | yes, a view the flow reads: the `total_time` arm scans all rounds and cannot be indexed |
| 15 | Views read whole by agents (`side_reports` 671, `not_applicable_answers` 699) | 699 | at most 0.018 | 13 | (17) 14 | measured upper bound | `side_reports` is a view the flow reads; `not_applicable_answers` is not |
| 16 | E4 reminder | 1.4 | 1 per turn, if set | 1.4 | (14) 88 | inferred | no |
| not scored | E6 `keep_warm` reads, R10, R8 (697 ms, 5 times per main-red matter) | | the count is not known or the event is rare | | | | |

Ranks 5 to 8 and 10 rest on the busiest 10 s of rounds (13 started), one window and not the usual one: at the average rate
(0.060 rounds in 10 s) each is under 20 ms. Ranks 1 to 4 and 9 hold only while a page is open and events keep coming.
The architect's long session and long work (ranks 6 and 8) grow with the work's age and not with the number of works: at
ten times the works they cost 35.5 and 25.8 ms, at ten times the long work 356 and 266 ms.

What each rank needs, as far as the evidence goes (the fix items are the architect's to open and split):

1. **P1, I5, I1, I2 (ranks 1 to 4).** `/items` is a regression of #312 and is item #316. The rest of the flush is the
   size of what the page reads again: `/system` (five reads, none of them bounded by the week it shows) and `/works`
   (every round of every work), unchanged by #312. These are interface and page changes, no flow change.
2. **R6, R2, R3, R1, R5 (ranks 5 to 8, 10).** `architect_time` needs its own bound by project and time (as #248 section 7 item 4
   said; #312 added no index for it and its time did not change); `own_rounds` of the architect's long session needs the index
   `rounds(session, work, n)` so that the order is served and nothing is sorted (a regression to mend with the
   index of #312); `resumed_at` reads the change log by `json_extract(row_key, '$.id')`, which no index serves, and
   wants a column or an index on the expression; reading a work's rounds by need is the one change here that touches what a
   cell is given. All engine or schema, none the flow.
3. **E1, E5, E3, E4.** What is left of E1 grows with the open and paused works; at the owner's own mix of works it may be
   smaller or larger than at ten times. `reload_config` compares the commit only after reading the files and is now
   the largest part of a turn at the field's size.
4. **R4 and the views read whole.** `work_counters`' `total_time` arm and the `json_each` over every round's output in
   `side_reports` and `not_applicable_answers` cannot be indexed away; they need a stored value (as #226 did for the
   counters), which changes a view the flow reads and so waits for the owner's approval.

## 5. What this report does not show

- The page's main-thread time and redraws (P1 to P3); the Rust code itself; the tables the research environment's `query`
  cannot read (`pings`, `round_calls`, `round_measures`, `cache_misses`); `keep_warm`'s session reads per turn (E6), the
  largest unknown behind the owner's observation of a full core: the measured parts of a turn are 0.5% of a core at the
  field's size and 1.3% to 1.7% at ten times now (53 to 57 and 132 to 175 ms per 10 s, inferred as a sum), against 1.7% to 2.2%
  and 11% to 17% in #248; they do not explain a full core, and what would is not among the measured.
- That the field has the effect of #313: the engine's own database has not taken the migration yet (section 1), so the
  loop that is running now still reads all 311 works not paused. The numbers of section 3 for E1 are those of a synthetic database
  built as the migration leaves it: the 54 works read by `active` are the recipe's 8 open engineer works and 46 open architect works, and `keep_warm` also reads the 14 paused ones (4 and 10). Whether the owner's real mix of open works is like that is not known.
- Rates: section 4's occurrences are those of #248, not measured again after the changes.
- Whether the engine's SQLite (`libsqlite3-sys` 0.39.1) chooses the same plans as Python's 3.45.1 for the two regressions of
  #312: it has no `sqlite_stat1` in either (inferred: nothing in `crates/db/src/lib.rs` runs `ANALYZE` or `PRAGMA
  optimize`), so it likely does. A test of the plan with `db::query_plan`, as #312 wrote for its statements, settles it.
