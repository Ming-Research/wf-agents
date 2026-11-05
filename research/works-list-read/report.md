# The list of works reads each work's time: does the read slow down?

A record of the measurement that item #240's acceptance condition A12 asks for. It is kept as evidence of that one change and
goes when `research/` is next cleaned of finished measurements; it states no requirement.

## The question

Item #240 puts a bar of where a work's time went in every row of the page's list of works. The list is read by
`api::works`, which loads every round of every work (`Light` rows) and sums them in memory. This change adds
`work_time` to that sum: for each round, whether an agent ran it in a session (a column of the same query), else the
kind of its cell, and the equal cells of the bar it touches. The work's run time, which the list already summed with a
lookup of each round's cell of its own, now comes from the same sum. Does the read of the list (`GET /api/v1/works`)
get slower, and by how much does its answer grow?

## The comparison

- Workload: `works_read.rs`, one test that builds in-memory databases of 300 works with 150 rounds each (45,000
  rounds, one in ten sent back), reads the list once untimed, then 20 times timed, and times the answer's JSON written
  by `serde_json`. It does so for four sets of rounds: `mixed` (five cells in turn: two tools', two models' in the
  work's session and the engine's; the figure A12 is judged by) and, to locate a change if one shows, rounds of one kind
  alone: `engine` (the engine's own steps, from which no bar is drawn), `models` and `tools`. For each it prints the
  minimum, median and maximum of the reads, the median of the writes and the size of the answer.
- Sides: the base (main at the commit this work's branch last took in, e541b375), its twin (the same commit, the
  control) and the candidate (this work's head), in interleaved pairs by the `measure` workflow, in an order that
  rotates. Each side runs the same harness file, copied from the candidate's checkout to
  `crates/api/tests/works_read.rs`, because the base has no such file; the harness uses only `api::works`, `api::Flows`,
  `db` and `flow`, which both have. Each side builds in release, in a target directory of its own: with one directory
  shared, cargo took the build of the side that ran first for the others, since their sources are older than that
  build, and the candidate ran the base's code (a run with a shared directory showed it: the base's and the
  candidate's answers were the same size, 126,661 bytes, though the candidate's carries a new field in each work).
- Machine: the workflow's runner (`ubuntu-24.04`, GitHub-hosted). Its numbers say nothing of the owner's machine; only
  the ratio between the sides of one run does.
- The figure of a pair is the median of the 20 reads of a set in microseconds, taken inside the test, so that building
  the code is not in it. The workflow's own wall times (`pairs.tsv`) include the build and are not used.

## What decides

For each pair, `twin - base` is that run's noise and `candidate - base` the change, in microseconds of the median read;
the 95% interval of the mean over the pairs (Student's t) is taken, and divided by the base's mean for the ratio.

- The run counts only if the candidate really ran the new code, which `body_bytes` shows (the candidate's answer is
  larger than the base's, the twin's the same size as the base's).
- The run counts only if the twin's interval holds 0: otherwise the run is noisier than a change can be told in, and is
  run again with more pairs.
- The change is a slowdown, and A12 fails, if the candidate's interval for `mixed` lies wholly above 0. It holds if the
  interval crosses 0 or lies below it.
- The other sets say where a slowdown comes from: `engine` has no round the bar is drawn from, so a slowdown there is in
  what every round costs (reading one more column, classifying a round); a slowdown in `models` or `tools` only is in
  drawing the bar.
- The size of the answer is read from `body_bytes` of the base and of the candidate and given as it is.

## Result

Measured on the workflow's runner (Linux, 4 processors), 30 interleaved pairs, the sides built each in its own target
directory, with this work's head dc5d2d33 as the candidate and e541b375 as the base. The figure of a pair and a set is
the median of its 20 reads, in microseconds, read from `command.log`. The intervals were worked out by hand from the
30 pairs (the difference of each side with the base, their mean, their standard deviation over 29 degrees of freedom
and Student's t of 2.045), not by a script: a run of `awk` or `node` on the engineer's machine needs the owner's
approval. The sets `models` and `tools` were run and printed but not worked out.

| set | base, mean read | twin - base, mean and 95% interval | candidate - base, mean and 95% interval | as a share of the base |
|---|---|---|---|---|
| mixed | about 39.2 ms | +185 us, -125 to +495 us | +1,488 us, +1,157 to +1,819 us | +3.8% (3.0% to 4.6%) |
| engine | about 38.4 ms | +53 us, -275 to +381 us | +341 us, +49 to +634 us | +0.9% (0.1% to 1.7%) |

- The new code ran: for `mixed` the candidate's answer is 187,345 bytes against the base's and the twin's 126,661
  (60,684 bytes more, 202 bytes for each of the 300 works, 48% more); for `engine` 182,845 against 125,161.
- The twin's interval holds 0 for both sets, so the run's noise is about 0.8% of a read and the runs count.
- The candidate's interval for `mixed` lies wholly above 0: the candidate was slower in 29 of the 30 pairs. This is a
  slowdown of the list's read, about 1.5 ms on 45,000 rounds, 33 ns for each round.
- Where it comes from: about 0.34 ms of it is in `engine`, where no bar is drawn, so it is what every round now costs
  (one more column read from the database, the passes over a work's rounds for its span and its bar); the other
  1.15 ms or so is drawing the bar of the rounds a model or the tools ran.
- The page's JSON written from the answer (`write_median_us`, taken in the same process) went from about 270 us in the
  base to about 360 us in the candidate, against a read of 39 ms.
- Not measured: a database whose rounds are as long as real ones (their inputs and outputs are `{}` here, since the list
  reads neither), and the owner's machine. The workload has 300 works of 150 rounds each, in the range the item names
  (hundreds of works, over a hundred rounds each).
