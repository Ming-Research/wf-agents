# What a look at a CI run costs GitHub's rate limit, measured on 2026-11-04

Item #289. These are numbers measured before #289 changed the engine, and the ones it rests on: #289 made a look at a
run one request, sent with the ETag of the last answer, every 5 s instead of every 20 s with `gh run list`. Where this
note says "before #289" it means the engine of that time. Whom it serves: the implementation of #289 and the reviews that
judge its claim of calls an hour below GitHub's limit. It is kept until the engine's way of looking at a run changes
again, and then replaced by the numbers of that change.

Each claim says how it is known: (measured) on the data of the day, (read) from source or documentation, (inferred)
from measurements, not tried. This is exploratory: the comparison and the result that would reject the proposal were
not written down before the measuring, so nothing here is a test of a prediction.

## 1. The answers

| Question | Answer |
|---|---|
| What did one look cost before #289? | 2 requests of the `core` limit: `gh run list --workflow <w> --commit <sha>` first resolves the workflow (`GET .../actions/workflows/<w>`), then lists its runs (read: gh source, `pkg/cmd/run/list/list.go`, `pkg/cmd/workflow/shared/shared.go`). 10 of them moved the counter by 20 (measured, section 3). |
| What does one look cost since #289? | One request, sent with the ETag of the last answer, every 5 s: charged to the limit only when GitHub answers `200` (the run changed), not when it answers `304` (section 5, last row). The rows below are the measurements it rests on. |
| Can one look cost 1? | Yes: `GET .../actions/workflows/<file name or id>/runs?head_sha=<sha>&per_page=20&exclude_pull_requests=true` moved the counter by 1 (measured). It takes a file name or an id, not the workflow's display name, which the project's configuration may hold (read: `crates/config/tests/on_demand.rs`, `fast = "fast"`). |
| Can a look cost nothing? | Yes while nothing changed: 8 requests with `If-None-Match` set to the ETag of the last answer got `304` and left the counter where it was (measured, section 3). The documentation says the same, for requests made with an `Authorization` header (read: "Best practices for using the REST API"). |
| What is the limit? | 5000 requests an hour on `core`, for the token the engine's `gh` uses (an OAuth token of the gh CLI app); secondary: 900 points a minute for REST, a `GET` is 1 point; the documentation does not say whether a `304` counts toward the secondary limit (read; measured for `X-Ratelimit-Limit: 5000`). |
| How many looks run at once? | At most 6 since 2026-11-03 10:00 UTC, for 210 s in all; the mean while any ran is 2.0 (measured, section 4). `ci` and `ci_before` of a work look on one thread each (read: `crates/engine/src/tools.rs`, the scope that spawns `ci_before` beside `ci`), so 6 slots both running make 12. |
| Does changing 20 s to 5 s alone fit under the limit? | Not for 12 looking at once, and not with a margin at the load measured (section 5). |

## 2. How it was taken

The engine's read-only `gh` tool ran `gh api ... -i`, which prints the response headers, among them
`X-Ratelimit-Used`, `X-Ratelimit-Remaining`, `X-Ratelimit-Reset`. The tool runs in the engine's process, so its `gh` is
the engine's `gh`. Calls ran one after another in the order listed, 0.5 to 1 s apart. The counter was read before and
after a block of calls of one kind; what the block was charged is the difference less 1 for the call that read it. The
engine's own traffic and the other works' share the counter, which is the noise of a reading: it was 0 in the first
three blocks below and 4 in the last.

Two counters were seen in the same half hour: `Used: 116`, reset at 1793835762, read at 23:15 UTC through
`.../git/ref/heads/first-version`, and the counter of every later reading (`Used: 1266` at 23:18:16, reset at
1793835295, window opened 22:34:55 UTC). The 10 `gh run list` calls of the first reading did not move the first
counter; the 10 of the second moved the second by 20. **Not explained.** Every reading used for a number below is of the
second counter, the one `gh run list` moved.

## 3. The cost of a look

Counter readings (measured, 2026-11-04 UTC, repository `Ming-Research/wf-agents`, workflow `fast.yml`):

| Block | Calls | `Used` read before | `Used` read after | Charged to the block | Per call |
|---|---|---|---|---|---|
| `gh run list --workflow fast.yml --commit <sha> -L 20 --json ...` | 10 | 1267 (23:18:17) | 1288 (23:18:23) | 1288 - 1267 - 1 = 20 | 2 |
| `gh api .../fast.yml/runs?per_page=1 -i` | 2 | 1266 (23:18:16) | 1267 (23:18:17) | 1267 - 1266 = 1 for the second | 1 |
| `gh api .../fast.yml/runs?head_sha=<sha>&... -i -H If-None-Match:<etag>`, the ETag matching, every answer `304` | 8 | 1382 (23:20:46) | 1383 (23:20:49) | 1383 - 1382 - 1 = 0 | 0 |
| the same with the header not matching (the tool's splitting of the command dropped the quotes of the ETag), every answer `200` | 8 | 1370 (23:20:43) | 1382 (23:20:46) | 1382 - 1370 = 12 for 8 calls: 8 and 4 of other traffic | 1 |

The `304` answer, as `gh api -i` prints it: the status line `HTTP/2.0 304 Not Modified`, the headers (`Etag: "<hex>"`
without the `W/` of the `200`, `X-Ratelimit-Used` unchanged), no body, and on stderr `gh: HTTP 304`; the exit status
is 1. The `200` answer carries `Etag: W/"<hex>"`; sending that value back whole matched. The ETag did not change
across 4 plain requests in 8 s, nor across the 16 requests above (the run list of a commit with no run).

The run objects of the `200` use `id`, `html_url` and `run_attempt` where `gh run list --json` says `databaseId`,
`url` and `attempt`; `status` and `conclusion` are the same (measured, a run in progress at 23:18).

## 4. The load

(measured; queries in section 8) The tools that wait for a run are `ci`, `ci_before`, `full_ci`, `merge`, `measure`;
their rows of `tool_runs` of 10 s or more, from 2026-11-03 10:00 UTC to 2026-11-04 23:20 UTC, gaps of an hour or more
left out. Seconds with that many of them running at once:

| At once | 0 | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|---|
| seconds | 9829 | 12940 | 12610 | 6838 | 2004 | 875 | 210 |

The mean while at least one ran is 2.04; the most is 6 (works 275, 280 and 274 on 2026-11-03 14:14 UTC, each with `ci`
and `ci_before`). The busiest hour, 2026-11-03 15:00 to 16:00 UTC, held 9359 s of such rows, a mean of 2.6.

The window of the second counter, 22:34:55 to 23:18:16 UTC (43.35 min): 6477 s of such rows, a mean of 2.5 at once.
It was charged 1266 requests, 29.2 a minute, 1752 an hour. At one look every 20 s and 2 requests a look, 6477 s are 324
looks and 648 requests, 51% of the 1266 (inferred: the rows' whole time counted as looking, which overstates it by the
push and the reading of the jobs; the rest, 618, is every other traffic of the token, among it about 45 of this
research's own). An hour of one poller at that rate is 180 looks and 360 requests.

## 5. What each way of looking costs an hour

Requests an hour (arithmetic on sections 3 and 4; the last row is inferred for the number of changes a run shows).

| Way of looking | One poller | 6 at once | 12 at once (6 slots, `ci` and `ci_before`) | At the load measured (2.5 at once) plus the other 855 an hour |
|---|---|---|---|---|
| before #289: `gh run list`, every 20 s | 360 | 2160 | 4320 | 898 + 855 = 1753 (measured: 1752) |
| `gh run list`, every 5 s | 1440 | 8640 | 17280 | 3600 + 855 = 4455 |
| `gh api` of the runs, every 5 s, 1 request | 720 | 4320 | 8640 | 1800 + 855 = 2655 |
| `gh api` with `If-None-Match`, every 5 s (what #289 made) | 720 made, charged only when the answer changed | | 8640 made, about 1200 to 2400 charged | 470 charged at most + 855 = 1325 |

The charged count of the last row is the answers that are `200`: the first look, the appearance of the run, and each
change of the run (queued, started, finished, and a change of its `updated_at`), at most 10 for a run of a few jobs
(inferred: the run object's fields; not counted on a real run). A poller is in at most about 17 runs an hour
(a `ci` lasts about 216 s, 3600 / 216 = 16.7) or 23 (a `ci_before`, about 160 s, 3600 / 160 = 22.5); 6 of each is 240
runs, 10 changes each is 2400 charged, 5 each is 1200. If the ETag never matched, the last row would cost what the one
above it does.

## 6. The wait the change saves

The time after the run ended is 8 to 53 s, median 18 s, mean 19.7 s (measured: `research/ci-speed-measured.md`,
section 4), made of a floor of about 8 s for reading the jobs and the results and the wait to the next look. A look at
a random moment of a wait of `T` seconds waits `T/2` on the mean: 10 s at 20 s, 2.5 s at 5 s, so about 7.5 s a call
(inferred from the shape, not isolated).

## 7. What this does not show

- One half hour of one token's counter, read through a tool that adds its own requests; the second counter is not
  explained, and whether the engine's looks are charged to it as the tool's are, is inferred from both being the same
  process's `gh`.
- 8 matching `304` answers on one query of a commit with no run. How often the ETag of a run in progress changes is not
  measured.
- Whether a `304` counts toward the secondary limit is not known; 12 pollers at one look every 5 s are 144 requests a
  minute, 16% of 900.
- The documentation asks a client to wait the `X-Poll-Interval` of a response where it has one. The responses of the
  runs endpoint carried none (measured on its plain answers, section 3); the one of the git ref carried 300.

## 8. To reproduce

- A counter reading: `gh api repos/<owner>/<repo>/actions/workflows/<file>/runs?per_page=1 -i`, the headers
  `X-Ratelimit-Used`, `X-Ratelimit-Reset`.
- A conditional look: the same with `-H 'If-None-Match: <the Etag of the last 200>'`; the status line says 304.
- The load, on the engine's database (`query` tool), epoch milliseconds:

```sql
-- Seconds with n of the waiting tools running at once (section 4).
WITH ev AS (
  SELECT started_at AS t, 1 AS d FROM tool_runs
   WHERE tool IN ('ci','ci_before','full_ci','merge','measure') AND ended_at IS NOT NULL
     AND ended_at - started_at >= 10000 AND started_at >= 1793700000000
  UNION ALL
  SELECT ended_at, -1 FROM tool_runs
   WHERE tool IN ('ci','ci_before','full_ci','merge','measure') AND ended_at IS NOT NULL
     AND ended_at - started_at >= 10000 AND started_at >= 1793700000000),
lv AS (SELECT t, sum(d) OVER (ORDER BY t, d ROWS UNBOUNDED PRECEDING) AS n,
              lead(t) OVER (ORDER BY t, d) AS t2 FROM ev)
SELECT n AS pollers, round(sum(t2 - t)/1000.0) AS seconds
  FROM lv WHERE t2 IS NOT NULL AND t2 - t < 3600000 GROUP BY n ORDER BY n;

-- Seconds of those rows in the window of the second counter (section 4).
SELECT round(sum(max(0, min(ended_at, 1793834296000) - max(started_at, 1793831695000)))/1000.0)
  FROM tool_runs
 WHERE tool IN ('ci','ci_before','full_ci','merge','measure') AND ended_at IS NOT NULL
   AND ended_at > 1793831695000 AND started_at < 1793834296000 AND ended_at - started_at >= 10000;
```
