# What reading the System view costs after it counts what the works waited for

Item 413 makes each read of `GET /api/v1/system` also work out what the works waited for (stops by cause, what CI took,
the slots held), over the span and for each week. This file holds what was measured of its cost, and what the numbers
can and cannot say. Whom it serves: the owner and whoever next changes how the System view is read, who judge by it
whether the cost stays (principle `measure-speed-and-cost`). It stays as the record of the measurement.

## Question, comparison and what would reject the change

- Question: does a read of the System view take more than 1.2 times as long as before, over many works?
- Comparison: the same test (`system-read-measured/system_read.rs`) on the code the change began from (main at
  `10114f2bf`) and on the change, in the same CI, as the page reads: the span cut into three weeks.
- Rejected if: the median of the reads after the change is above 1.2 times the median before (acceptance condition 16).

## How it was measured

The project declares no measurement to `run_tests` (`wf-agents.toml` has no `[ci.measure.<name>]`), so the engine's
measure workflow could not be named. The test was put into `crates/web/tests/routes/` of two commits that are in no
branch (the code before and the code after, each with the test file and a line `mod system_read;` in `main.rs`) and
run by `run_tests` on each, several runs of each at once. It adds 100 trial works through the engine, each merged, and in
the last round 5 more that were added and not let on (so that a part is worked out from the flow installed, as for every
work that has not ended); then, five times, in a new engine: one read, then twenty-one more, each timed in the test with
`Instant`. The test ends in a failure whose words are the times, which is how a CI run gives them back. The CI machine
runs several jobs at once, so the runs overlapped one another and a lint job.

## Results, in milliseconds

First read of an engine (works every work's part out) and the median of 21 reads after it, of 5 engines in each run.

| round | side | runs | first read, all engines: median / least | warm read, all engines: median / least |
|---|---|---|---|---|
| 1, change before the first change of how it reads | before | 2 | 10.4 / 5.3 | 3.1 / 1.6 |
| 1 | after | 2 | 7.5 / 6.1 | 2.4 / 2.2 |
| 2, 100 merged works, change after sorting the slots once | before | 3 | 6.1 / 5.3 | 1.8 / 1.6 |
| 2 | after | 3 | 10.3 / 6.0 | 3.3 / 2.0 |
| 3, 100 merged and 5 not ended, change as handed in | before | 3 | 13.0 / 5.6 | 3.3 / 1.7 |
| 3 | after | 3 | 13.6 / 6.1 | 2.9 / 2.0 |

Round 3 is the code handed in. Its times by run, the median of 21 warm reads of each engine:

- before: `[3.5, 1.7, 3.6, 3.0, 3.1]`, `[1.8, 3.6, 3.1, 1.7, 4.0]`, `[3.3, 3.6, 3.3, 3.4, 3.3]`;
- after: `[2.9, 2.6, 2.8, 4.5, 4.4]`, `[2.1, 2.1, 2.1, 2.0, 2.0]`, `[4.2, 4.1, 3.8, 4.2, 3.8]`.

## What the numbers say

- A median over the runs cannot be trusted to 20 %: the same code gave warm medians from 1.6 to 6.2 ms across runs, the
  runs being on different runners and overlapping one another. The ratio of medians was 0.77, 1.83 and 0.88 in the three
  rounds, which is noise.
- The least time of each side is steady across rounds. After over before: first read 1.15, 1.13 and 1.09 times; warm read
  1.38, 1.25 and 1.18 times (1.6 to 2.2 ms, 1.6 to 2.0 ms, 1.7 to 2.0 ms). The code of round 3 differs from that of round 2
  in reading what holds every work at once only where a part is worked out or a work has not ended, and then only the
  agents' wait of it (before, every read worked out the slots taken, the turns and the acts' host as well), and in sorting
  the slots held once for each read instead of once for each window; round 1 had neither.
- So the change adds about 0.3 to 0.4 ms to a read over 105 works and about 0.5 ms to the first read. Over 105 works that
  is a fifth to a sixth of what the read took, and a read of 400 works with many rounds each is longer to the same degree
  on the parts that are read again for every work (the parts kept for works that ended are not).
- What was not measured: the cost over a copy of the installed database, which the CI does not have and an agent does not
  read; the recomputation of the parts of many works that have not ended when what holds every work at once changes; and
  a read on the machine of the installation. Whoever has the installed engine can time `GET /api/v1/system` there before
  and after the install.

## Reproducing

Put `system-read-measured/system_read.rs` into `crates/web/tests/routes/` of a checkout, add `mod system_read;` to the
`main.rs` there, and run the test (`system_read::the_system_is_read_over_a_hundred_merged_works`) in the CI; its failure
gives the times. It must be run in the CI, not on the machine the agents run on.
