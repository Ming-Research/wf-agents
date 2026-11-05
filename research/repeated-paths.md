# Paths that run again and again, and what they cost as the data grows, measured on 2026-11-05

Item #248. The engine's database was read as it stood on 2026-11-05 (28.4 days of work: 365 works, about 14,850 rounds,
55,915 changes); the paths were timed on a synthetic database built to that size and to ten times it, in one run on
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
| What is the biggest cause | Finished works are never marked `ended`: 252 of the 316 works with `ended IS NULL` are done (at their graph's last cell), and every pass of the loop that lists "the works that go on" loads each of them (read: `crates/engine/src/ending.rs:88-102` sets `ended` only for a work stopped before its end; measured by `query`). One pass that loads the 304 unpaused unended works takes 26.0 ms now and 262 ms at ten times the works (measured); a turn of the loop makes at least 4 such passes (section 3, E1). |
| Next | Every write of the engine first scans the whole change log for its next transaction number: 33.1 ms per write now (55,915 change rows), 334 ms at ten times, inside the write lock (E2, measured, read). At least 18,845 writes that changed a tracked table have been made (measured: the transactions in `changes`). |
| What the loop costs a turn | Adding the measured parts by the number of times the source calls each: 167 to 220 ms per turn now, 1.1 to 1.7 s at ten times (E1; the sum is inferred from measured parts). A turn runs when a job ends and at least every 10 s (read: `crates/wf-agents/src/main.rs:1046`). |
| What the owner's page costs the interface | The two reads every page holds (`/works` and `/conversations`, section 3 I2) cost 77 ms of database time per flush now and 2.6 s at ten times; the page flushes at most once in 3 s while events keep coming (read: `client/src/live.ts`). One flush of everything an event batch can name costs 438 ms and 2.8 MB of result now, 6.2 s and 19 MB at ten times (measured; the bytes are the rows' text, not the JSON the interface sends). |
| What grows by itself | The architect's longest work (3,330 rounds, 22% of all rounds) never ends and is read whole by every step that composes a round of it: 27 ms and 29 MB per read now, 294 ms at ten times its rounds (measured at scale 1L10: 10.7 to 11.0 times; about 290 MB is ten times the bytes, inferred). Its page, `GET /works/{id}`, is 29.8 MB (R1, I4, measured). |
| Is anything already fixed | `graph_of` caches the cut graph (read: `crates/engine/src/lib.rs:1090-1107`); the page batches events into one flush in 3 s and reads nothing while hidden (read: `client/src/live.ts`). `work_counters` no longer parses the compiled flow: the counter is kept on the round (`rounds.counts`), and the remaining cost is a scan of all rounds (E7). The parse of the compiled flow that #226 names now happens only for a sent-back round and takes 2.0 ms, the same at ten times (measured; 322 of about 14,850 rounds). |
| What is flat | The paths of section 4 are flat at ten times: the lookups bounded by a work's key, one work's rounds, one round. They show the program tells a flat path from a growing one. |
| What was not measured | The page's main-thread time and redraws (CI has no browser: inferred from the source, section 3 P1 to P3); the tables `pings`, `round_calls`, `round_measures`, `cache_misses`, which the research environment's `query` tool cannot read (built empty); the Rust code itself (section 2). The per-turn session reads of `keep_warm` are the largest unknown (E6). |

## 2. How the numbers were taken

- **The data.** `repeated_paths.py` builds `db/schema.sql` into a file and fills it from the figures it holds in
  `RECIPE`, `CELLS`, `TOOLS` and `CHANGES`: the field's row count of every table (measured with `query` on 2026-11-05),
  each round's cell, size and whether it ran an agent, each work's rounds and state, the change log by table, the
  matters by kind, the compiled flow at 1.7 MB. Row texts are random words, so the sizes are the field's and the contents
  are not. At scale 1 the program checks every table against the number it built and the main tables against the field
  within 5%; it ended `DONE ok` in all nine processes. Scale 10 has ten times the works, items, rounds, tool runs,
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
  so it is not read finer than that; their growth at ten times (5.5 to 5.7, 13.3 to 13.8 and 28 to 31 times) is far above
  it, and the class stands. Every other path's interval contains 1, or has no spread (1.000 to 1.000); the widest are
  paths under 0.1 ms (`engine.round.item_merged` 0.68 to 1.74, `engine.step.take_if_past` 0.70 to 1.29), where the
  clock is the limit (checked by a search of the results file for intervals wholly on one side of 1).
  Page cache: the machine had the files in memory after the build; the engine's connections use SQLite's default 2 MiB
  cache, as the program does, so a table over 2 MiB is read through the operating system on every pass at scale 10. That
  is why some small views grow more than ten times (`view.item_acceptance` 59 to 68 times): it is a real effect of the
  default cache, measured, and not a quadratic term.
- **Classes** were written before the run: flat when the interval of the ratio contains 1; grows below linear when it lies
  above 1 and below the scale factor; linear when it lies above 1 and reaches the factor; superlinear when it lies wholly
  above it. An interval from 10.03 to 10.12 at factor 10 is therefore "superlinear" by the rule and is a linear path with
  the cache effect above; this report calls paths within 10.0 to 10.7 "linear" in words and keeps the table's class in the file.
- **The first full run built the matters with a guessed mix of kinds.** The field's mix was read afterwards (`report`
  271, `set_straight` 180, `owner_words` 131, `open_question` 127, ...) and the run repeated; only the run on the corrected
  mix is used here. The check that the program's own count check fails on a short build was not made to fail.

## 3. The list, by severity

Severity is the cost of one occurrence at today's size times how often it occurs, and how it grows; the first rows
multiply (a pass repeated, a write that every round makes). Times are milliseconds at scale 1, then at scale 10, then the
ratio's interval. "Item" names an item that already covers the path.

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
| R1 | The architect's longest work: `work::rounds` (every round of the work with its input and output), called by `steps.rs:1036` (composing every round), `:1346`, `:1626`, `:1705` (only `.pop()`), `:1819`, `inputs.rs:508` | at least once per round of that work | the work's own rounds: the whole work is read for each new round, so O(n^2) per work | 27.2 ms, 29.3 MB | at scale 10 27.8 (flat); at 1L10 294.5 (10.68 to 10.99), ~290 MB | measured, call sites read |
| R1b | The same work: `open_round` at the start of every step; `api.work.arch` for its page | every step; every read of its page | the work's rounds | 3.3 (arch), 73.8 ms and 29.8 MB (page) | at 1L10 34.0 (10.16 to 10.83), 767.5 (10.34 to 10.46) | measured |
| R2 | `own_rounds` / `of_cli_session` (`sessions/src/round.rs:84,111`; callers `held.rs:270`, `warm.rs:296`): every round of a CLI session. `rounds.session` has no index, so the plan scans all rounds | every agent round (composing it), and E6 | all rounds of the database | engineer 13.9 ms; architect 28.2 ms and 18.6 MB | 137.3 (9.79 to 9.97); 152.2 (5.4 times: the session's rows do not grow) | measured; plan read |
| R3 | `work::resumed_at` (`steps.rs:209`, `failed.rs:145`): `json_extract(row_key, '$.id')` cannot use `changes_by_row`, so every `works` change is read | twice per agent round | changes of the table `works` (15,774) | 4.1 | 41.2 (10.04 to 10.19) | measured, read |
| R4 | The `work_counters` view read whole (cell A1's input, the owner's page): the `rounds_counts` arm is an index scan, the `total_time` arm scans all rounds. One work's counter is flat | per A1 round and per page read of counters | rounds | 40.5 whole; 0.017 for one work | 408.4 (10.01 to 10.13); flat | measured; readers inferred. #226 |
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
| I4 | `/works/{id}` of the architect's longest work: 3,330 rounds with their whole input and output (29.8 MB); the correlated `min(n)` per round makes it quadratic in the work | the work's rounds | 73.8 | flat at 10; 767.5 at 1L10 | measured |
| I5 | `/items` (9.3 over `/works`), `/observations` 17.1, `/architect` notebook 3.0 (every version's full text, no limit), `/flows` 1.0 (a scan of `works` for each flow version: works x versions), `/architect/rounds` 22.8 (sorts all architect rounds), item level changes 7.1 (scans every `items` change) | items, observations, notebook versions, works x versions | as listed | 124.3, 180.7, 52.3 (17 to 18), 92.0 (93 to 96), 231.6, 65.6 | measured |
| I6 | `api::events` after each write: two point lookups per changed row (`events.rs:39`) | rows a write changes | 4.1 for 200 rows | 4.2 (flat) | measured |

### Page (`client/src`): not timed

| # | Path | What is known | How |
|---|---|---|---|
| P1 | `live.ts`: every event names keys; a flush reads again once in 3 s what they name. A `round` event names works, items, messages, system and observations; a `work` event names works, items, system | The reads of one flush are the statements of I1, I2, I5 that the open page holds: 438 ms and 2.8 MB of result for the largest set now, 6.2 s and 19 MB at ten times, which is more than one flush period, so reads would pile up if the reader did not run one at a time per path | measured (SQL); the flush set read from `named()` |
| P2 | `reader.ts`: the whole body of each read is parsed and compared with the last when its text differs | Each read of the architect's long work is 29.8 MB of JSON to parse | read |
| P3 | Lists drawn whole: the works table, conversation list, rounds list, system sections (inventory of the page) | Redraw count and main-thread time grow with the number of rows; the page re-renders all rows on each status event | inferred; no browser in CI |

### Database views, read whole

Every view was read with `SELECT count(*)`; the full list is in `results-2026-11-05.txt`. The agents' own SQL reaches them
through the `query` tool (4,378 runs so far, measured), so a view read whole is a cost an agent can cause; the engine's
cell inputs read most of them with `WHERE work = ?` or `WHERE item = ?`, which a primary-key prefix bounds (inferred;
only `work_counters` for one work was timed, and it is flat). At scale 1, then at 10: `side_reports` 71.8 -> 727.9,
`not_applicable_answers` 75.3 -> 760.4 (both `json_each` over every round's output), `work_counters` 39.1 -> 395.5,
`work_followups` 21.8 -> 217.5, `advisor_rulings` 16.2, `adversarial_directions` 15.9, `item_research` 13.9,
`sent_back_problems` 13.9, `observations_all` 13.3, `policy_current` and `policy_draft` 13.0, `work_set_aside` 12.7,
`work_progress` 12.0, `item_verifications` 12.0 (each about 10 times at ten), `conversation_shown` 9.0 -> 1,067.1,
`conversation_entries` 6.6 -> 1,040.7, `ready_items` 5.2 -> 82.6, `item_blocked_by` 4.3 -> 71.0. The other 25 views take
under 1 ms and six of them grow more than ten times only because their table passes the default cache.

## 4. Flat, and why they are

Flat at ten times (the interval of the ratio contains 1): `take_if_past` 0.025 ms, `stop_asked` 0.011, one work's counter
0.017, the work's rounds of an engineer (85 rounds) 0.61 and of the longest engineer work (326) 2.9, `rounds_at` 0.34,
`item_merged` 0.020, the time limit 0.99, the compiled-flow lookup for a sent-back round 2.0, `api.round` 0.036,
`api.work.eng` 2.0, the forked job's database open 2.2, the 200 event lookups 4.1, the content search 3.6, the architect's
long work at ten times the works (27.8, 72.5: the work itself does not change). They are bounded by a primary key of the
work or by one row, and are the contrast that shows the program separates them.

## 5. What is fixed or has an item

- Fixed: the cut graph's cache (read), the page's batching and hidden-tab guard (read), `work_counters`'s parse of the
  compiled flow (the counter is on the round).
- Items: #226 (work counters: its text names the parse, which is gone; the scan of all rounds in `total_time` remains,
  R4) and #220 (the page hang) overlap I2 and P1; both were open when the brief was written and their state was not read again.

## 6. What this report does not show

- The page's time and redraws, and the Rust code's own time (section 2).
- The count of calls per turn of the `keep_warm` session reads (E6) and of the `round_measures` read (R10).
- Whether the engine at the owner's machine uses a full core because of E1 to E6: the sum of the measured parts is
  167 to 220 ms per 10 s of an idle engine, about 2% of a core now and 11 to 17% at ten times; more when jobs end often.
  The owner's observation of a full core is not explained by the measured parts alone, which is what E6 may hold.
- That the program's count check fails on a short build (section 2).

## 7. What to do next

Fix items, in this order, each as small as its cause (the architect splits them and merges the small ones):

1. Stop listing finished works: set `ended` when a work completes its end cell, or list works by SQL that leaves them out.
   The views `ready_items` and `open_items` define "done" as `ended IS NULL AND at IN ('E1', '20', 'end')`, so the change
   needs a design that moves those too (E1).
2. Number transactions without a scan: an index on `changes(tx)` or a counter row (E2).
3. Index the columns the plans scan: `rounds(session)`, `rounds(cell)`, `interjections(matter)`, `engine_notices(ask)`,
   `architect_matters(work)`, `architect_matters(asked)`, `blockers(ask)`, `works(flow_version)` (R2, R5, R7, I3, I5).
4. Read a work's rounds by need, not whole, for the composing of a round and the page (R1, I4).
5. Build `conversation_entries` for one conversation, not for all (I3), and bound `/system`, `/works` and `/items` by
   what the page shows (I1, I2, I5).
6. Compare the configuration's commit before reading its files (E3).
7. Record how long each turn of the loop takes, and how many calls each pass made, in the engine's records (Article 4), so
   that E6 and the owner's full core are measured and not inferred.
8. After each fix, run the program again (the command at the top) and put the fixed path's scale test in the code that
   owns it; remove the program when each path has one.
