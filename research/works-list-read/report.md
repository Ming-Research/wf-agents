# The list of works reads each work's time: does the read slow down?

A record of the measurement that item #240's acceptance condition A12 asks for. It is kept as evidence of that one change and
goes when `research/` is next cleaned of finished measurements; it states no requirement.

## The question

Item #240 puts a bar of where a work's time went in every row of the page's list of works. The list is read by
`api::works`, which loads every round of every work (`Light` rows) and sums them in memory. This change adds
`work_time` to that sum: for each round, the kind of its cell and the equal cells of the bar it touches. The work's run
time, which the list already summed with a lookup of each round's cell of its own, now comes from the same sum, so that
each round's cell is looked up once, as before. Does the read
of the list (`GET /api/v1/works`) get slower, and by how much does its answer grow?

## The comparison

- Workload: `works_read.rs`, one test that builds an in-memory database of 300 works with 150 rounds each (45,000
  rounds; five cells in turn: two tools', two models' and the engine's; one round in ten sent back), reads the list once
  untimed, then 30 times timed, and times the answer's JSON written by `serde_json`. It prints the minimum, median and
  maximum of the reads, the median of the writes and the size of the answer.
- Sides: the base (main at the commit before this work, 92ee6625), its twin (the same commit, the control) and the
  candidate (this work's head), in interleaved pairs by the `measure` workflow, in an order that rotates. Each side runs
  the same harness file, copied from the candidate's checkout to `crates/api/tests/works_read.rs`, because the base
  has no such file; the harness uses only `api::works`, `api::Flows`, `db` and `flow`, which both have. Each side builds
  in release, in a target directory of its own: with one directory shared, cargo took the build of the side that ran
  first for the others, since their sources are older than that build, and the candidate ran the base's code (the
  first two-pair run showed it: the base's and the candidate's answers were the same size, 126,661 bytes, though the
  candidate's carries a new field in each work).
- Machine: the workflow's runner (`ubuntu-24.04`, GitHub-hosted). Its numbers say nothing of the owner's machine; only
  the ratio between the sides of one run does.
- The figure of a pair is the median of the 30 reads in microseconds, taken inside the test, so that building the code is
  not in it. The workflow's own wall times (`pairs.tsv`) include the build and are not used.

## What decides

For each pair, `twin / base` is that run's noise and `candidate / base` the change. The 95% interval of the mean of the
logarithms of each ratio over the pairs (Student's t) is taken.

- The run counts only if the candidate really ran the new code, which `body_bytes` shows (the candidate's answer is
  larger than the base's, the twin's the same size as the base's).
- The run counts only if the twin's interval holds 1: otherwise the run is noisier than a change can be told in, and is
  run again with more pairs.
- The change is a slowdown, and A12 fails, if the candidate's interval lies wholly above 1. It holds if the interval
  crosses 1 or lies below it.
- The size of the answer is read from `body_bytes` of the base and of the candidate and given as it is.

## Result

Measured on the workflow's runner (Linux, 4 processors), 30 interleaved pairs, the sides built each in its own target
directory; the figure of a pair is the median of its 30 reads, in microseconds, read from `command.log`. The
intervals were worked out by hand from the 30 pairs (the differences in microseconds of each side with the base, their
mean, their standard deviation over 29 degrees of freedom and Student's t of 2.045), not by a script: a run of `awk` or
`node` on the engineer's machine needs the owner's approval.

| | reads (median of the pairs' medians) | against the base, mean of the 30 differences | 95% interval of that mean | as a ratio to the base |
|---|---|---|---|---|
| base | about 37.0 ms | | | |
| twin (the noise) | about 37.0 ms | -3 us | -188 to +182 us | 0.995 to 1.005 |
| candidate | about 37.0 ms | -18 us | -210 to +174 us | 0.994 to 1.005 |

- The new code ran: the candidate's answer is 187,345 bytes against the base's and the twin's 126,661 (60,684 bytes more,
  202 bytes for each of the 300 works, 48% more), so that none of the sides took another's build.
- The twin's interval holds 1 and is as wide as the candidate's: the run's noise is about +/-0.5% of a read, and a
  slowdown of that size or more would have been seen. The candidate's interval crosses 1, so no change of the read is
  detected; A12 holds.
- The page's JSON written from the answer (`write_median_us`, taken in the same process) went from 240 to 304 us in the
  base (mostly 245 to 265) to 318 to 360 us in the candidate (mostly 320 to 350): about 75 us more, against a read of
  37 ms.
- Not measured: a database whose rounds are as long as real ones (their inputs and outputs are `{}` here, since the list
  reads neither), and the owner's machine. The workload has 300 works of 150 rounds each, in the range the item names
  (hundreds of works, over a hundred rounds each).
- The first run, of two pairs, only showed the harness built and ran; its three sides shared one build and are not used.
