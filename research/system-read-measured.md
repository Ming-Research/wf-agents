# What reading the System view costs after it counts what the works waited for

Item 413 makes each read of `GET /api/v1/system` also work out what the works waited for (stops by cause, what CI took,
the slots held), over the span and for each week. This file holds what was measured of its cost, and what the numbers
can and cannot say. Whom it serves: the owner and whoever next changes how the System view is read, who judge by it
whether the cost stays (principle `measure-speed-and-cost`). It stays as the record of the measurement.

## Question, comparison and what would reject the change

- Question: does a read of the System view take more than 1.2 times as long as before, (a) over many works that have
  ended, and (b) over many that have begun and not ended, each with many waits?
- Comparison: the same tests (`system-read-measured/system_read.rs`) on main (the code the change began from) and on the
  change, in the same CI, as the page reads: the span cut into three weeks.
- Rejected if: the least of the engines' median reads after the change is above 1.2 times the least before, for the first
  read and the warm read of (a), and for the warm read and the read after a record of (b) (acceptance condition 16). The
  first read of (b) was not named in the criterion; it is given below as measured.
- If (b) was above it, the parts of the works going on were to be kept by what they are made from and measured again; if
  still above, the owner was to be asked whether those parts may be decided only at the middle of each cell, as the bar
  of a work going on is (decision `a-live-bar-asks-at-its-cells-middles`), which makes their numbers less exact.

## How it was measured

The project declares no measurement to `run_tests` (`wf-agents.toml` has no `[ci.measure.<name>]`), so the engine's
measure workflow could not be named. The test file was put into `crates/web/tests/routes/` of two commits that are in no
branch (the code before and the code after, each with the file and a line `mod system_read;` in `main.rs`) and run by
`run_tests` on each, two or three runs of each at once. Each test builds its works through the engine, then, five times, in a
new engine: one read (the first, which works every work's part out), then twenty-one more, each timed in the test with
`Instant`. Each ends in a failure whose words are the times, which is how a CI run gives them back. The CI machine runs
several jobs at once, so the runs overlapped one another and a lint job.

- (a) `the_system_is_read_over_a_hundred_merged_works`: a hundred works, each merged, and five added and not let on, so
  that a part is worked out from the flow installed, as for every work that has not ended.
- (b) `the_system_is_read_over_works_going_on_with_many_waits`: a hundred cancelled works, five not begun, and ten that
  have begun and are going on, each with two hundred waits: the size of the largest works in the owner's store, three of
  which had more than 150 waits on 2026-11-21 (research/flow-versions/bar-cost.md), here ten at once. A round of each
  fails and the owner lets it on, two hundred times. Three reads are timed: the first; the median of twenty-one with
  nothing changed between them (every part kept stands); and the median of twenty-one each after every work going on got a
  record, as records come while the page is read (the part of each is worked out again).

## Results, in milliseconds

Each cell is the least of the engines' medians over the runs (five engines in each run), for the reason below. Main for the first
three rounds of (a) was `10114f2bf`; for the later rounds it was `d6af233bd`.

(a) a hundred merged works, five not ended; first read / warm read:

| round | the change | before | after | after over before |
|---|---|---|---|---|
| 1 | every read asked for everything that holds every work at once; the slots held sorted for each window | 5.3 / 1.6 | 6.1 / 2.2 | 1.15 / 1.38 |
| 2 | the slots held sorted once for each read; the waits every work shares read once | 5.3 / 1.6 | 6.0 / 2.0 | 1.13 / 1.25 |
| 3 | what holds every work at once read only where a part is worked out or a work has not ended | 5.6 / 1.7 | 6.1 / 2.0 | 1.09 / 1.18 |
| 4 | as round 3, with main of `d6af233bd` merged in; a second set of three runs of the same code gave 6.4 / 2.1, 1.16 / 1.24 times | 5.5 / 1.7 | 5.9 / 2.0 | 1.07 / 1.18 |
| 5 | what a work going on decided is kept from read to read | 5.5 / 1.7 | 6.0 / 2.0 | 1.09 / 1.18 |
| 6 | as handed in | 5.5 / 1.7 | 5.9 / 1.9 | 1.07 / 1.12 |

(b) ten works going on, two hundred waits each; first read / warm read / read after a record:

| round | the change | before | after | after over before |
|---|---|---|---|---|
| 4 | the parts kept by the revision of the work and what holds every work at once; each part's stretches decided anew | 34.8 / 35.0 / 40.1 | 89.6 / 35.3 / 101.1 | 2.57 / 1.01 / 2.52 |
| 5 | what a work going on decided at the places of its records is kept from read to read | 34.8 / 35.0 / 40.1 | 90.6 / 34.7 / 53.1 | 2.60 / 0.99 / 1.32 |
| 6 | as handed in: the moment after a time and the stretch of what ran it falls in found by halving, and the return of a round found from lists made once | 34.8 / 35.0 / 40.1 | 81.3 / 34.2 / 43.3 | 2.34 / 0.98 / 1.08 |

Round 6 is the code handed in. The median of 21 reads of each of the five engines, by run:

- (a) before, first read: `[5.8, 5.9, 5.7, 5.5, 5.5]`, `[5.7, 5.6, 5.5, 5.8, 15.9]`, `[5.5, 5.6, 5.6, 6.3, 17.2]`; warm read:
  `[1.7, 1.7, 1.7, 1.7, 1.7]`, `[1.7, 1.7, 1.7, 1.7, 3.3]`, `[1.7, 1.7, 2.0, 1.8, 1.9]`.
- (a) after, first read: `[19.4, 6.9, 6.4, 17.3, 6.7]`, `[6.0, 5.9, 5.9, 5.9, 6.4]`, `[6.7, 6.7, 6.5, 5.9, 6.4]`; warm read:
  `[4.0, 2.2, 2.1, 3.5, 2.1]`, `[1.9, 1.9, 1.9, 1.9, 2.0]`, `[2.0, 2.1, 2.0, 1.9, 2.0]`.
- (b) before, first read: `[38.2, 35.5, 57.3, 56.0, 36.2]`, `[37.5, 35.2, 59.7, 35.8, 35.0]`, `[37.6, 34.9, 60.8, 34.8, 84.3]`;
  warm read: `[36.1, 36.0, 59.4, 52.7, 35.8]`, `[39.4, 35.0, 58.2, 36.3, 35.0]`, `[35.9, 35.8, 56.9, 36.3, 55.7]`; read
  after a record: `[41.8, 40.9, 66.6, 42.6, 40.5]`, `[42.6, 40.3, 52.4, 41.7, 40.1]`, `[41.3, 40.6, 67.6, 68.6, 68.2]`.
- (b) after, first read: `[85.4, 84.1, 83.5, 83.5, 83.2]`, `[85.9, 84.4, 81.3, 82.7, 81.3]`, `[85.6, 83.3, 83.7, 83.8, 104.8]`;
  warm read: `[34.9, 37.2, 34.7, 34.6, 34.6]`, `[36.9, 35.3, 34.5, 35.9, 37.5]`, `[34.7, 34.8, 34.2, 34.3, 35.0]`; read
  after a record: `[44.8, 44.3, 43.3, 44.3, 44.3]`, `[45.3, 44.9, 44.2, 43.7, 44.7]`, `[45.4, 45.6, 44.4, 44.5, 43.6]`.

## What the numbers say

- A median over the runs cannot be trusted to 20 %: the same code gave warm medians from 1.6 to 6.2 ms across runs in (a),
  and 34 to 71 ms in (b), the runs being on different runners and overlapping one another. The ratio of medians was 0.77,
  1.83 and 0.88 in the first three rounds of (a), which is noise. Noise only adds time, so each side is judged by the least
  of its engines' medians, which stood steady across runs: in the last round of (a) 5.5 ms against 5.9 ms and 1.7 ms against
  1.9 ms, over three runs of five engines each side.
- The resolution of the times is 0.1 ms, a seventeenth of a warm read in (a). On the code of round 4 the least warm read
  was 2.0 ms in one set of three runs and 2.1 ms in another, 1.18 and 1.24 times before's: one inside the criterion and
  one outside it, which the resolution cannot tell apart. The code handed in gives 1.9 ms, 1.12 times.
- (a) The change adds about 0.2 ms to a read over 105 works and about 0.4 ms to the first read. The parts kept for works that
  ended are not worked out again.
- (b) A read with nothing changed costs what it did before (0.98 times). A read after every work going on got a record costs
  1.08 times what it did, 3 ms more over ten works with two hundred waits each. It was 2.52 times before the parts of the
  works going on kept what they decided, and 1.32 times with it: a part worked out again decided every one of its waits
  again, each by the records before it, so that the cost grew with the square of the waits (as the bar's did,
  research/flow-versions/bar-cost.md); what is decided at a place that records follow stays decided, and what is decided
  after the latest record is decided again. The rest came from two changes made together, not measured apart: the moments and the
  stretches of what ran are halved for each time asked, where each was looked through, and the return of each round is found
  from lists made once, where all returns were looked through for each round.
- (b) The first read costs 2.34 times before's: 81 ms against 35, about 4.6 ms more for each work with two hundred waits.
  It is the cost of deciding each of the waits once, by the records before it. The criterion did not name it and it is not
  below it; it is paid once for each work going on each time the engine starts (and for a part worked out for the first
  time), not at each read.
- What was not measured: the cost over a copy of the installed database, which the CI does not have and an agent does not
  read; a read on the machine of the installation, where the crates are built with optimisation and every time is shorter
  (the CI builds them without); works going on whose project has many turns held by others, or many waits in which the agents
  could not run: the part of a work going on looks through each of these for each time it asks, and the works of these
  tests hold none. Whoever has the installed engine can time `GET /api/v1/system` there before and after the install.

## Reproducing

Put `system-read-measured/system_read.rs` into `crates/web/tests/routes/` of a checkout, add `mod system_read;` to the
`main.rs` there, and run the tests (`system_read::the_system_is_read_over_a_hundred_merged_works` and
`system_read::the_system_is_read_over_works_going_on_with_many_waits`) in the CI; each failure gives the times. They must
be run in the CI, not on the machine the agents run on. The second takes up to a minute and a half on the code before the
change.
