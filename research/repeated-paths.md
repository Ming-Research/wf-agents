# Paths that run again and again, and what they cost as the data grows, measured on 2026-11-05

Item #248. The engine's database was read as it stood on 2026-11-05 (28.4 days of work: 365 works, 14,843 rounds,
55,886 changes: the figures the synthetic databases were built to; a later read the same day found about 14,850 rounds
and 55,915 changes); the paths were timed on a synthetic database built to that size and to ten times it, in one run on
GitHub's `ubuntu-24.04` runner. This report measures and lists; it changes no engine, interface, page or database file.
Whom it serves: the architect, who opens the fix items from it and orders them, and the owner, who wants to know how many
works the system can carry before it is slow. It is kept until its fixes are in and the program below is run again.

Files, all in `research/`:

- `repeated-paths.md` (this report);
- `repeated-paths/repeated_paths.py`: the program. `run` builds the synthetic databases from `db/schema.sql` and times
  every path and every view; `summarize` turns the log into the tables of `repeated-paths/results-2026-11-05.txt`
  (the run behind this report, kept as it printed). The program's header says who calls it and when it is removed;
- `repeated-paths/results-2026-11-05.txt`: the tables of the run: for every path its rows and bytes, its time at the
  field's size (scale 1), at the field's size with the architect's longest work stretched ten times (scale 1L10) and at
  ten times the field (scale 10), the ratio of each to scale 1 with its 95% interval, the noise, and the query plans.

To run it again (the `measure` tool, `measure.yml`, three pairs, about 25 minutes of which the first process builds the
databases; the last `summarize` output of the log is the file above):

```
python3 -I research/repeated-paths/repeated_paths.py run --scales 1,1L10,10 | tee -a "$RUNNER_TEMP/rp-all.log"; \
python3 -I research/repeated-paths/repeated_paths.py summarize "$RUNNER_TEMP/rp-all.log"
```

Each claim says how it is known: (measured) by that run, (read) from the source or the records, (inferred) from
those, not tried.

## 1. The answers

| Question | Answer |
|---|---|
| In what order | By the ranking table at the head of section 3: the cost of one occurrence at ten times the data times the occurrences in 10 s at a rate measured from the engine's records. First are the writes (E2), the page's reads (P1, I1, I2) and the conversation read (I3); the loop's passes (E1) are the largest cost that is always there. |
| What is the biggest cause of the loop's load | Finished works are never marked `ended`: 252 of the 316 works with `ended IS NULL` are done (at their graph's last cell), and every pass of the loop that lists "the works that go on" loads each of them (read: `crates/engine/src/ending.rs:88-102` sets `ended` only for a work stopped before its end; measured by `query`). One pass that loads the 304 unpaused unended works takes 26.0 ms now and 262 ms at ten times the works (measured); a turn of the loop makes at least 4 such passes (section 3, E1). |
| The worst burst | Every write of the engine first scans the whole change log for its next transaction number: 33.1 ms per write now (55,886 change rows), 334 ms at ten times, inside the write lock (E2, measured, read). 18,867 writes that changed a tracked table have been made, on average 0.077 in 10 s, and 87 in the busiest 10 s (measured: the transactions in `changes`), so at ten times the lock would be held 29 s in that 10 s. |
| What the loop costs a turn | Adding the measured parts by the number of times the source calls each: 167 to 220 ms per turn now, 1.1 to 1.7 s at ten times (E1; the sum is inferred from measured parts). A turn runs when a job ends and at least every 10 s (read: `crates/wf-agents/src/main.rs:1046`). |
| What the owner's page costs the interface | The two reads every page holds (`/works` and `/conversations`, section 3 I2) cost 77 ms of database time per flush now and 2.6 s at ten times; the page flushes at most once in 3 s while events keep coming (read: `client/src/live.ts`). One flush of the eight reads listed in the results file (the `/works`, `/items`, `/conversations` and `/system` reads; not `/observations` and not the pages of open items, so a lower bound) costs 438 ms and 2.8 MB of result now, 6.2 s and 19 MB at ten times (measured; the bytes are the rows' text, not the JSON the interface sends); adding `/observations` (17.1 and 180.7 ms) makes 455 ms and 6.4 s (sum of medians). |
| What grows by itself | The architect's longest work (3,330 rounds, 22% of all rounds) never ends and is read whole by every step that composes a round of it: 27 ms and 29 MB per read now, 294 ms at ten times its rounds (measured at scale 1L10: 10.7 to 11.0 times; about 290 MB is ten times the bytes, inferred). Its page, `GET /works/{id}`, is 29.8 MB (R1, I4, measured). |
| Is anything already fixed | `graph_of` caches the cut graph (read: `crates/engine/src/lib.rs:1090-1107`); the page batches events into one flush in 3 s and reads nothing while hidden (read: `client/src/live.ts`). `work_counters` no longer parses the compiled flow: the counter is kept on the round (`rounds.counts`), and the remaining cost is a scan of all rounds (R4). #226 and #220 are both merged (section 5). The parse of the compiled flow that #226 names now happens only for a sent-back round and takes 2.0 ms, the same at ten times (measured; 322 of about 14,850 rounds). |
| What does not grow | The paths of section 4 do not grow with the data: the lookups bounded by a work's key, one work's rounds, one round. They show the program tells a flat path from a growing one. |
| What was not measured | The page's main-thread time and redraws (CI has no browser: inferred from the source, section 3 P1 to P3); the tables `pings`, `round_calls`, `round_measures`, `cache_misses`, which the research environment's `query` tool cannot read (built empty); the Rust code itself (section 2). The per-turn session reads of `keep_warm` are the largest unknown (E6). |

## 2. How the numbers were taken

- **The data.** `repeated_paths.py` builds `db/schema.sql` into a file and fills it from the figures it holds in
  `RECIPE`, `CELLS`, `TOOLS` and `CHANGES`: the field's row count of every table (measured with `query` on 2026-11-05),
  each round's cell, size and whether it ran an agent, each work's rounds and state, the change log by table, the
  matters by kind, the compiled flow at 1.7 MB. Row texts are random words, so the sizes are the field's and the contents
  are not. At scale 1 the program checks every table against the number it built and the main tables against the field
  within 5%; it ended `DONE ok` in all nine processes. The views are not listed in the program: it reads their names from
  `sqlite_master`, and the results file has a row for each of the 46 views of `db/schema.sql` in each comparison. Scale 10 has ten times the works, items, rounds, tool runs,
  changes and flow versions, each work and row as large as at scale 1: the same engine run ten times as long. Scale 1L10
  is scale 1 with the longest architect work at 33,300 rounds, since that work grows with age whatever the number of
  works. The scale-10 database is 7.9 GB, scale 1 is 0.8 GB.
- **Assumed, because `query` cannot read the table:** sessions 300, engine notices 250, conversation reads 30 (the change
  log holds 1,784, 318 and 1,722 changes of them). Rows of `pings`, `round_calls`, `round_measures` and `cache_misses`
  are not built, so what reads them is timed as reading an empty table. The configuration repository for
  `reload_config` is built with 16 files (install, one project, two shared, twelve rules); the engine's data directory
  cannot be read from here, and the cost is linear in the number of files.
- **The statements.** Each path is the SQL the code runs, copied from the source (the call sites are in section 3),
  run with the same `sqlite3` rules on the same file; a path that is a loop in the code (the pass over the works) is the
  same statements in the same order in a loop. A few statements are shortened where the original was too long to copy,
  and say so in the program (`api.system.work_changes` leaves out the OR terms of its filter, so it is a lower bound).
  **Not the Rust code:** the time is SQLite's plus Python's driver; the Rust code calls the same statements and is
  probably faster per statement (inferred), and it parses the rows' JSON, which is not timed. The ratios between scales
  are what the report rests on; the milliseconds are not the engine's milliseconds. SQLite is 3.45.1 here; the engine's is
  the one `libsqlite3-sys` 0.39.1 bundles (its SQLite version was not read), so a plan may differ.
- **Runs.** One process builds the databases (once, cached for the others) and times each path on all three scales in turn
  (one warm-up run, then 5 timed runs, or 3 when the first took over 2 s, or 1 when over 20 s) and keeps the median. Nine
  processes: three pairs of `base`, `twin` and `candidate` of the same commit in a rotating order (run
  <https://github.com/Ming-Research/wf-agents/actions/runs/42091751715>, commit 95ac342f; the program has changed since
  in one statement only, `engine.round.opens`, whose separator is now an ASCII slash instead of a middle dot so that the
  file is ASCII, which changes no number). The ratio of a path is the ratio of its medians
  at two scales in the same process; the table gives its 95% interval over the nine (Student's t, on the logarithms). The
  noise is the twin's time over the base's at scale 1 in each pair, its interval over the three pairs. For three paths
  the interval lies wholly below 1 (`api.architect.matters` 0.953 to 0.979, `api.items_extra` 0.927 to 0.969,
  `view.architect_queue` 0.874 to 0.953): their scale-1 time differs by up to 13% between processes of the same commit,
  so it is not read finer than that. By the criterion written before the run (a path whose twin-to-base interval excludes
  1 is noisier than a measured path may be) these three are **unresolved**: this report draws no class and no
  conclusion for them, and the class the results file prints for them is withdrawn. Their growth at ten times (5.5 to
  5.7, 13.3 to 13.8 and 28 to 31 times) is shown only as it was observed; a rerun would settle them. Every other path's interval contains 1, or has no spread (1.000 to 1.000); the widest are
  paths under 0.1 ms (`engine.round.item_merged` 0.68 to 1.74, `engine.step.take_if_past` 0.70 to 1.29), where the
  clock is the limit (checked by a search of the results file for intervals wholly on one side of 1).
  Page cache: the machine had the files in memory after the build; the engine's connections use SQLite's default 2 MiB
  cache, as the program does, so a table over 2 MiB is read through the operating system on every pass at scale 10. That
  is why some small views grow more than ten times (`view.item_acceptance` 59 to 68 times): it is a real effect of the
  default cache, measured, and not a quadratic term.
- **Classes** are the acceptance criterion of this item (A3) applied to the interval of the ratio of the time at ten times
  to the time at scale 1: wholly below 1, the path does not grow ("faster" in the file); containing 1, no growth is
  detected ("flat"); containing 10, linear; wholly above 10, superlinear. The program was written with the same rule and
  one addition for the gap A3 leaves, an interval wholly above 1 that does not reach 10: "grows, below linear". The class
  column of the results file is what the program printed with this rule; a path is called flat in this report only where
  that column says "flat". An interval from 10.03 to 10.12 at factor 10 is "superlinear" by the rule; it is a linear path
  with the cache effect above, and the report says "linear" for paths within 10.0 to 10.7 and names the cause (a stated
  departure from the letter of A3, since the excess is the cache and not the code). The program's class for the three
  paths whose noise interval excludes 1 (below) is withdrawn: they are unresolved. The program does not yet print that;
  the file ends with a note that says it, since the log it was printed from is the measurement artifact and python3 does
  not run on the machine that wrote this report.
- **The first full run built the matters with a guessed mix of kinds.** The field's mix was read afterwards (`report`
  271, `set_straight` 180, `owner_words` 131, `open_question` 127, ...) and the run repeated; only the run on the corrected
  mix is used here. The check that the program's own count check fails on a short build was not made to fail.

## 3. The list, by severity

Severity is the cost of one occurrence times how often it occurs, and how it grows. The ranking below fixes it as one
number: **database time per 10 s at ten times the data** = the time of one occurrence at scale 10 (scale 1L10 for the
architect's long work) times the occurrences in 10 s. The occurrences are taken from rates that are measured or read:
a turn of the loop at least once in 10 s (read: `main.rs:1046`); a write 0.077 in 10 s on average and 87 in the busiest 10 s
(measured from the 18,867 transactions of `changes`); an agent round starts 0.060 in 10 s on average and 13 in the busiest
10 s (measured from `rounds`; 22% of them are the architect's longest work's); a flush of an open page at most once in 3 s,
3.3 in 10 s, while events keep coming (read: `live.ts`). Where a rate is inferred (a path run once per round it is not
shown to run on every round) the row says so. A path is ranked by the larger of its numbers at the average and at the
busiest rate; the turn and the flush have one rate each, the floor. The column "Today" is the same product at scale 1.

| Rank | Path | One occurrence at 10x (ms) | Occurrences in 10 s | ms in 10 s at 10x | Today (ms) | Rate known by |
|---|---|---|---|---|---|---|
| 1 | E2 transaction number | 334 | 87 at the busiest (0.077 average) | 29,058 (26 on average) | 2,880 | measured |
| 2 | P1 one page flush (the eight reads of the results file) | 6,182 | 3.3 | 20,400: more than the 10 s, so the page's reads never rest | 1,445 | read |
| 3 | I1 `/system` (part of 2) | 3,456 | 3.3 | 11,405 | 1,160 | read |
| 4 | I2 `/works` and `/conversations` (part of 2) | 2,592 | 3.3 | 8,554 | 254 | read |
| 5 | I3 reading one conversation | 1,015 | 3.3 while a conversation is open | 3,350 | 22 | read |
| 6 | R6 `blockers::architect_time` | 204 | 13 at the busiest | 2,652 | 280 | inferred (one per step of a waiting work) |
| 7 | R2 `own_rounds` | 137 to 152 | 13 at the busiest | 1,781 to 1,976 | 181 to 367 | inferred (one per agent round) |
| 8 | E1 the passes over unended works | 1,100 to 1,700 | 1 per turn | 1,100 to 1,700 | 167 to 220 | read (counts), measured (parts) |
| 9 | R3 `resumed_at` (twice per agent round) | 82 | 13 at the busiest | 1,071 | 107 | inferred |
| 10 | R1 the architect's long work read whole | 294 (290 MB) | 2.9 at the busiest (22% of rounds) | 854 | 79 | inferred |
| 11 | I5 `/architect/rounds` (`/observations` 181, `/items` 124, unresolved) | 232 | 3.3 while open | 764 | 75 | read |
| 12 | R5 `rounds_of_matter` | 178 | 2.9 at the busiest (architect rounds) | 517 | 50 | inferred |
| 13 | E5 `ready_items`, `slots_free_history` | 90 | 1 per turn | 90 | 5.6 | read |
| 14 | E4 reminder | 88 | 1 per turn, if the installation sets the interval | 88 | 0.4 | inferred |
| 15 | E3 `reload_config` (twice) | 62 | 1 per turn | 62 | 62 | read |
| 16 | R4 `work_counters` whole | 408 | 0.06 at the busiest (A1 rounds are 70 of 14,850 rounds); the page's reads of it are not counted | 25 | 2.5 | inferred |
| 17 | Views read whole by agents (`side_reports` 728, `not_applicable_answers` 760) | 760 | at most 0.018 (all 4,378 `query` runs over 28 days) | 14 | 1.4 | measured upper bound |
| not scored | E6 (`keep_warm` reads), R8 (per main-red matter, 5 times, 763), R10 | | the count is not known or the event is rare | | | |

Ranks 6, 7, 9, 10, 12 and 16 rest on the busiest 10 s of rounds (13 started), which was one window, not the usual one: on
the average rate (0.060 rounds in 10 s) each is under 20 ms. Ranks 2 to 5 hold only while a page is open and events keep
coming. Times in the tables below are milliseconds at scale 1, then at scale 10, then the ratio's interval. "Item" names
an item that already covers the path.

### Engine, every turn of the loop (a turn: a job ended, or 10 s passed)

| # | Path and where | Runs | Grows with | Now | At 10x | How known |
|---|---|---|---|---|---|---|
| E1 | The passes that load every work not ended: `Engine::active` (called by `outdated` at `schedule.rs:217` and by `dispatch` at `:286`) and `slots_by_machine` (= `active` plus the wait of each active engineer work; called by `free_slots` in `architect.rs:348`, `schedule.rs:364` and `main.rs:840`) and the `keep_warm` scan of every unended work (`warm.rs`). Each loads the work (4 statements) and asks `done` | 2 `active` + 1 to 3 `slots_by_machine` + 1 `keep_warm` per turn | works not ended: finished works are never ended (252 of 316) | `active` 26.0, `slots_pass` 26.5, `keep_warm_scan` 27.3; a turn 167 to 220 | 262.0 (10.03 to 10.12), 268.2, 276.9 (10.07 to 10.17); a turn 1.1 to 1.7 s | passes measured; counts read at the lines named; the sum inferred |
| E2 | `Db::write` takes the next transaction number: `SELECT coalesce(max(tx), 0) + 1 FROM changes` (`crates/db/src/lib.rs:504`), inside the immediate transaction, so under the write lock | every write: the loop, every step, every model call and tool use that writes (read); at least 18,845 so far (measured) | rows of `changes` (nothing deletes from it; no index on `tx`) | 33.1 | 333.9 (10.00 to 10.16) | measured, read |
| E3 | `reload_config`: `config::committed` runs `git rev-parse`, `git ls-tree -r` and one `git show` per file, and compares the commit only after (`lib.rs:1174`, `config/src/lib.rs:1300-1321`) | twice per turn (`schedule.rs:207`, `main.rs`) | files of the configuration, not the data | 30.8 per call (16 files assumed) | same | measured on an assumed file count |
| E4 | The reminder (`reminders.rs:11`): reads `conversation_entries` whole for the oldest waiting ask | once per turn when the installation sets a reminder interval and none is due (inferred from the early return at `interval_ms <= 0`) | asks x matters/notices (indexes missing on `engine_notices.ask`, `architect_matters.asked`, `blockers.ask`) | 0.36 | 88.3 (243 to 253) | measured; frequency inferred |
| E5 | `start_ready` and `queue_due` read `ready_items` and `slots_free_history` | once per turn | items, rounds of dependency works | 5.4 and 0.15 | 84.2 (15.3 to 16.0), 5.3 (33 to 36) | measured; frequency read |
| E6 | Per unended work, `keep_warm::due_of` -> `choose::plan` -> `of_cli_session` -> `own_rounds` (see R2 for the cost of one call) | per work with a session and a prompt cache, per turn | works not ended x all rounds | not measured as a loop | not measured | one call measured (R2); the number of calls per turn not counted: the largest unknown |

### Engine, per write, step and round

| # | Path and where | Runs | Grows with | Now | At 10x | How known |
|---|---|---|---|---|---|---|
| R1 | The architect's longest work: `work::rounds` (every round of the work with its input and output), called by `steps.rs:1036` (composing every round), `:1346`, `:1626`, `:1705` (only `.pop()`), `:1819`, `inputs.rs:508` | at least once per round of that work | the work's own rounds: the whole work is read for each new round, so O(n^2) per work | 27.2 ms, 29.3 MB | at scale 10 27.8 (1.01 to 1.04: not growing with the works); at 1L10 294.5 (10.68 to 10.99), ~290 MB | measured, call sites read |
| R1b | The same work: `open_round` at the start of every step; `api.work.arch` for its page | every step; every read of its page | the work's rounds | 3.3 (arch), 73.8 ms and 29.8 MB (page) | at 1L10 34.0 (10.16 to 10.83), 767.5 (10.34 to 10.46) | measured |
| R2 | `own_rounds` / `of_cli_session` (`sessions/src/round.rs:84,111`; callers `held.rs:270`, `warm.rs:296`): every round of a CLI session. `rounds.session` has no index, so the plan scans all rounds | every agent round (composing it), and E6 | all rounds of the database | engineer 13.9 ms; architect 28.2 ms and 18.6 MB | 137.3 (9.79 to 9.97); 152.2 (5.4 times: the session's rows do not grow) | measured; plan read |
| R3 | `work::resumed_at` (`steps.rs:209`, `failed.rs:145`): `json_extract(row_key, '$.id')` cannot use `changes_by_row`, so every `works` change is read | twice per agent round | changes of the table `works` (15,774) | 4.1 | 41.2 (10.04 to 10.19) | measured, read |
| R4 | The `work_counters` view read whole (cell A1's input, the owner's page): the `rounds_counts` arm is an index scan, the `total_time` arm scans all rounds. One work's counter does not grow with the data (0.89 to 1.00) | per A1 round and per page read of counters | rounds | 40.5 whole; 0.017 for one work | 408.4 (10.01 to 10.13); one work's does not grow | measured; readers inferred. Overlaps #226 (merged; the scan remains, section 5) |
| R5 | `rounds_of_matter` (`work.rs:212`): the architect's rounds at a cell, found without an index on `rounds.cell` | per architect round with a place input (inferred from the call chain) | all rounds | 17.1 | 178.4 (10.28 to 10.45) | measured |
| R6 | `blockers::architect_time` | per step of a work waiting on the architect (inferred) | architect rounds in the project | 21.5 | 204.3 (9.41 to 9.55) | measured |
| R7 | `interjections_words` and `asked_matters`: scans of `interjections` and of `architect_matters` for a work | per round that checks verdicts or reviews (inferred) | interjections, matters | 0.031 and 0.084 | 0.64 (20 to 21) and 4.1 (46 to 50) | measured |
| R8 | `main_watch::merges`: parses the output of every round to find one commit | per main-red matter, 5 times (read) | all rounds | 75.6 | 762.6 (10.05 to 10.15) | measured |
| R9 | `ci` results of the work, parsed several times a return (`checks.rs`); `fact_basis_ran` | per return | the work's tool runs | 0.36 and 1.2 | 0.47 and 1.5 (at most 1.3 times) | measured |
| R10 | `round_measures` rows of the whole graph read for each round with a guard (`guard.rs:575`) | per agent round | measured rounds of the graph | not measured | not measured | read only; the table cannot be read by `query` |

### Interface (`GET`s; a read follows each event batch the open page names)

| # | Path and where | Grows with | Now | At 10x | How known |
|---|---|---|---|---|---|
| I1 | `/system` (`system.rs`): `works_of` 66.6, `rounds_since` 109.0 (a scan of all rounds for the last week), `counted` 67.3, `work_changes` 63.9, `item_events` 44.8 | works, rounds, changes: none is bounded by the week it shows | 351.6 | 3,456.5 (the five: 678.1, 623.8 (5.6 times), 691.4, 798.9 (12.4 to 12.6), 664.3 (14.6 to 15.0)) | measured |
| I2 | `/works` (58.5; 7,727 rows, 479 kB: every round of every work, the `waiting` and `recorded` columns that the summary does not use) and `/conversations` (18.6) are held by the top bar of every page | works, rounds, messages | 77.1 | 2,592 (`/works` 10.5 to 10.6 times; `/conversations` 105 to 109 times) | measured; the top bar read (`TopBar.tsx`, inventory) |
| I3 | `conversation_entries` is a UNION of 12 selects built whole for any read of one conversation (the plan is `CO-ROUTINE conversation_entries / COMPOUND QUERY`); some arms scan unindexed columns (`interjections.matter`, `engine_notices.ask`, `architect_matters.asked`, `blockers.ask`) | messages x matters x interjections | `/conversation` of one work 5.7; of the architect 5.8; view 6.6 | 1,014.7 (175 to 183); 1,018.4 (171 to 175); 1,040.7 (155 to 160) | measured; the unindexed arms inferred from the SQL shape |
| I4 | `/works/{id}` of the architect's longest work: 3,330 rounds with their whole input and output (29.8 MB); the correlated `min(n)` per round makes it quadratic in the work | the work's rounds | 73.8 | 72.5 at 10 (0.97 to 1.00: not growing with the works); 767.5 at 1L10 | measured |
| I5 | `/items` (9.3 over `/works`; unresolved, noise: section 2), `/observations` 17.1, `/architect` notebook 3.0 (every version's full text, no limit), `/flows` 1.0 (a scan of `works` for each flow version: works x versions), `/architect/rounds` 22.8 (sorts all architect rounds), item level changes 7.1 (scans every `items` change) | items, observations, notebook versions, works x versions | as listed | 124.3, 180.7, 52.3 (17 to 18), 92.0 (93 to 96), 231.6, 65.6 | measured |
| I6 | `api::events` after each write: two point lookups per changed row (`events.rs:39`) | rows a write changes | 4.1 for 200 rows | 4.2 (1.01 to 1.02: not growing with the data) | measured |

### Page (`client/src`): not timed

| # | Path | What is known | How |
|---|---|---|---|
| P1 | `live.ts`: every event names keys; a flush reads again once in 3 s what they name. A `round` event names works, items, messages, system and observations; a `work` event names works, items, system | The reads of one flush are the statements of I1, I2, I5 that the open page holds. The eight reads the program times (`/works`, `/items`, `/conversations`, and the five reads of `/system`) cost 438 ms and 2.8 MB of result now, 6.2 s and 19 MB at ten times, which is more than one flush period, so reads would pile up if the reader did not run one at a time per path. This is a lower bound: a `round` event also names `observations` (17.1 and 180.7 ms) and, for an open item page, `item:*`; those are not in the 438 ms, and the item pages were not timed (only item level changes, I5) | measured (SQL); the flush set read from `named()` and the tests that pin it (`page.test.ts`) |
| P2 | `reader.ts`: the whole body of each read is parsed and compared with the last when its text differs | Each read of the architect's long work is 29.8 MB of JSON to parse | read |
| P3 | Lists drawn whole: the works table, conversation list, rounds list, system sections (inventory of the page) | Redraw count and main-thread time grow with the number of rows; the page re-renders all rows on each status event | inferred; no browser in CI |

### Database views, read whole

Every view was read with `SELECT count(*)`; the full list is in `results-2026-11-05.txt`. The agents' own SQL reaches them
through the `query` tool (4,378 runs so far, measured), so a view read whole is a cost an agent can cause; the engine's
cell inputs read most of them with `WHERE work = ?` or `WHERE item = ?`, which a primary-key prefix bounds (inferred;
only `work_counters` for one work was timed, and it does not grow with the data: 0.89 to 1.00). At scale 1, then at 10: `side_reports` 71.8 -> 727.9,
`not_applicable_answers` 75.3 -> 760.4 (both `json_each` over every round's output), `work_counters` 39.1 -> 395.5,
`work_followups` 21.8 -> 217.5, `advisor_rulings` 16.2, `adversarial_directions` 15.9, `item_research` 13.9,
`sent_back_problems` 13.9, `observations_all` 13.3, `policy_current` and `policy_draft` 13.0, `work_set_aside` 12.7,
`work_progress` 12.0, `item_verifications` 12.0 (each about 10 times at ten), `conversation_shown` 9.0 -> 1,067.1,
`conversation_entries` 6.6 -> 1,040.7, `ready_items` 5.2 -> 82.6, `item_blocked_by` 4.3 -> 71.0. The other 25 views take
under 1 ms and six of them grow more than ten times only because their table passes the default cache.

## 4. Flat, and why they are

Flat at ten times (the interval of the ratio contains 1; the class column of the results file says "flat"; times in ms):
`take_if_past` 0.025, `stop_asked` 0.011, the work's rounds of an engineer (85 rounds) 0.61 and of the longest engineer
work (326) 2.9, `item_merged` 0.020, the time limit 0.99, the compiled-flow lookup for a sent-back round 2.0, the content
search 3.6, `open_round` of the architect 3.3.

Not growing with the data, but the interval of the ratio excludes 1, so the file does not say "flat": it changes by at most
25% in either direction. Below 1 (the file says "faster"): one work's counter 0.017, `rounds_at` 0.34, the forked job's
database open 2.2 (0.98 to 0.99), `/works/{id}` of the architect's long work 72.5 (0.97 to 1.00). Above 1 (the file says
"grows, below linear"): `api.round` 0.036 (1.12 to 1.23), `api.work.eng` 2.0 (1.01 to 1.04), the 200 event lookups 4.1
(1.01 to 1.02), the architect's long work read whole at ten times the works 27.8 (1.01 to 1.04; the work itself does not
change). A change of this size is not a rate of growth the research would act on, and is read as "not growing with the data".

They are bounded by a primary key of the work or by one row, and are the contrast that shows the program separates them
from the growing paths.

## 5. What is fixed or has an item

Read from the git history of main and the engine's items on 2026-11-05: #226, #220 and #16 are merged and no item of
the list is open for them (`open_items` holds none of #16, #220, #226, #303); the item they asked for is therefore not
there to fix the paths below.

- Fixed (read): the cut graph's cache (`graph_of`); the page's batching of events into one flush in 3 s and its guard for
  a hidden tab (`99d2d3d`); #226 (merged as `f5225f0`): the counter of the row a round took is kept on the round
  (`rounds.counts`), so `work_counters` no longer parses the compiled flow for every round; #220 (merged as `0ac7ecc`):
  the page's reader keeps one slot per path, with one read at a time per path and at least a second between starts, an
  answer with the last text changes nothing, the architect's notebook, handled matters and item changes are read apart
  from the rest (`/architect/panel`), a conversation gives its latest 100 entries, and unchanged entries are not drawn
  again; #16 (merged as `36e8928`): the design check and the table review now ask how a cost grows with the load.
- Still slow after those merges (measured on the synthetic database of 2026-11-05, which has those merges' code and
  schema), and covered by no item: R4, the scan of all rounds in `total_time` that #226 left (40.5 ms, 408 ms at ten
  times); I2, `/works` answering every round of every work (58.5 ms, 479 kB) that every page holds, and `/conversations`;
  I1, `/system`; P1, the flush set, whose reads #220 limits in number and pace but not in size.
- Not settled by reading: whether #220's window of 100 entries stops the database from building the whole of
  `conversation_entries` (I3). The statement timed is the view filtered by conversation, without the window, so I3's
  1,015 ms is the cost if the window is applied after the view is built, and an upper bound otherwise. The window and the
  architect panel were not timed.

## 6. What this report does not show

- The page's time and redraws (P1 to P3), and the Rust code's own time (section 2). The page was not measured because the
  fast CI has no browser unless a file named `*.browser.test.tsx` is run, and this work may add no file outside
  `research/`. It can be measured by a browser test that draws the page against the interface's answers on the synthetic
  databases and counts the reads, the bytes and the redraws of a batch of events. The Rust code's time can be measured by
  the same statements called from a test of the crate that owns each path, at both scales.
- The count of calls per turn of the `keep_warm` session reads (E6): nothing in the engine's records counts them. It
  can be measured once the engine records each turn's passes and calls (section 7, item 8), or by a counter in a debug
  build run on a copy of the field's database.
- The `round_measures` read of R10 and the tables the research environment's `query` cannot read: the row counts are
  not known, so the program builds them empty. It can be measured once the owner reads the counts of those tables on the
  engine's machine and puts them in `RECIPE`.
- Whether the engine at the owner's machine uses a full core because of E1 to E6: the sum of the measured parts is
  167 to 220 ms per 10 s of an idle engine, about 2% of a core now and 11 to 17% at ten times; more when jobs end often.
  The owner's observation of a full core is not explained by the measured parts alone, which is what E6 may hold.
- That the program's count check fails on a short build (section 2).

## 7. What to do next

Fix items, in the order of the ranking table of section 3 (the rank of each is in brackets), each as small as its cause (the
architect splits them and merges the small ones):

1. Number transactions without a scan: an index on `changes(tx)` or a counter row (E2, rank 1).
2. Bound the page's reads by what the page shows: `/system`, `/works`, `/conversations`, `/items`, and the size of one
   flush (P1, I1, I2, I5; ranks 2 to 4 and 11). `/works` should not answer every round of every work.
3. Build `conversation_entries` for one conversation, not for all, and check what #220's window does (I3, rank 5).
4. Index the columns the plans scan: `rounds(session)`, `rounds(cell)`, `interjections(matter)`, `engine_notices(ask)`,
   `architect_matters(work)`, `architect_matters(asked)`, `blockers(ask)`, `works(flow_version)` (R2, R5, R7, I3, I5; ranks
   6, 7, 12). R6 needs its own bound by project and time.
5. Stop listing finished works: set `ended` when a work completes its end cell, or list works by SQL that leaves them out.
   The views `ready_items` and `open_items` define "done" as `ended IS NULL AND at IN ('E1', '20', 'end')`, so the change
   needs a design that moves those too (E1, rank 8; the largest cost the loop always has, and the cause of every pass).
6. Read a work's rounds by need, not whole, for the composing of a round and the page (R1, I4; rank 10).
7. Compare the configuration's commit before reading its files (E3, rank 15), and the other per-turn reads (E4, E5).
8. Record how long each turn of the loop takes, and how many calls each pass made, in the engine's records (Article 4), so
   that E6 and the owner's full core are measured and not inferred.
9. After each fix, run the program again (the command at the top) and put the fixed path's scale test in the code that
   owns it; remove the program when each path has one.
