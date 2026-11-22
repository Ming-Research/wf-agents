# What the times of the tests in CI show

For: whoever builds how test times are recorded and judged (item 47), and its reviewers.
Kept: while the design of test times stands as built; deleted when it changes, git keeping it.
Written: 2026-11-22. These are measurements of that day on the 24 latest successful runs of `ci.yml` on main
(2026-11-22), taken from the artifact `test-results` of each run with `gh run download`, and read with
`research/test-times-measured.py`. They were taken to see what a line from history can and cannot judge; nothing was
written down beforehand that a result would reject, so they are exploratory.

## What was found

- One test reads differently from run to run. For the 1652 tests above 5 ms in each of the last 19 runs, the slowest
  run of a test over its fastest is 5.0 at the median and 8.3 at the 90th percentile (over the last four runs alone:
  2.3 and 4.3). A test of the walks of random histories read 49 s in one run and 83 s in another, on code that was the
  same but for what the commits between changed.
- The sum of the tests' times of a run moved between 372 s and 664 s over the 24 runs, while the number of tests grew
  from 2335 to 2500 (165 tests). Whatever the whole sum says is the noise of the machine's load; what a change adds is seen in the
  tests it adds, not in the sum.
- Judged against their own 19 earlier times, 11 to 32 of about 2300 tests a run (0.5 to 1.4 %) were past their line
  (`2 x largest - median`); judged against 3 earlier times, 222 of 2233 (10 %). A line from few samples is too close, which is why
  `history.samples` is 19. About 170 to 190 tests of each run had fewer than 19 earlier times and were not judged.
- The line of the times of every test, which is what a stop that must suit every test could use, is 202 s: the
  largest time is 101 s and the median 0.022 s. A line of every test's times does not flag a test of seconds: that
  needs the sum of what a change adds, against the sums earlier changes added.
- The whole `ci.yml` run on main took a median of 88 to 120 s a day from 2026-11-15 to 2026-11-22, the 22nd's 27 runs
  being the slowest day (median 119 s, most 228 s). On `next` (the old main) the runs of 2026-11-10 took 1 to 3 minutes and those of
  2026-11-11 4 to 8 minutes, as the item says.
- What a merge adds is mostly small (4 s or less on 21 of 23 merges, counting the tests the run before did not report) and the
  line of the earlier additions (`2 x largest - median`) stands near 44 s after one merge that added 22 s: a second
  22 s batch would not be past it. That batch was five console-page tests of 2 to 6 s, each slower than 98.7 to 99.3 %
  of the tests before it. The slowest 14 of 2500 tests (4.99 s and more) hold half of the suite's test time in the
  latest run (21 tests, 3.77 s, twelve runs before). So a line on earlier additions says nothing of creep by moderate
  batches, while where a test stands among the suite's tests does.
