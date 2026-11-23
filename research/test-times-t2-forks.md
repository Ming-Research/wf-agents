# What T2 does with the new question 4, tried on forks of a past round

For: whoever changes the words of T2, `test_times` or the flow's rules about a test's time (item 47), and its reviewers.
Kept: while T2's question 4 and the datum `test_times` stand as built; deleted when either changes, git keeping it.
Written: 2026-11-23, from the answers the owner handed back. The forks were asked by the owner with
`wf-agents fork 82 74 --round 1 --question <file>`, on the session of the first round of run 74 of the cell T2 of work
82. The words of T2's task were not changed in that session: the question file tells the fork what question 4 reads
from now on and gives it one `test_times` text. The answers are kept whole in the engine's data directory, under
`rounds/82/74-1.fork.<n>.answer`.

## What was asked

Each fork was given this passage, with `<TEXT>` replaced by one of the three texts below and the last words of the
second sentence of the change replaced as the cases say:

> You review a different change from the one in your session; judge only by this. From now on question 4 of your task
> reads: 'Is the time of the change's tests worth a question? Judge it from `test_times`: it says what the CI measured
> of the tests the work's runs reported and where they stand among the project's tests. Where it shows time worth asking
> about, list the tests in one point, each with what it must show it catches that no other test catches and why it
> cannot be smaller, faster or folded into another. A test in the diff that `test_times` does not list was not
> measured: say what must be shown. Where `test_times` says the host gives no test's time, ask nothing about time.' The
> change adds the five tests of a console page. [Or: three small tests. Or: some tests.] `test_times` reads: <TEXT>.
> Answer question 4 only: write the point you would raise in `test_review`, or say that you raise none and why.

The texts are those the datum writes from flow/data/test_times.toml; the test names are examples.

1. The console page's five tests, 6080, 5200, 4300, 3400 and 2100 ms, 21080 ms together, 4 % of the 500000 ms a merged
   check typically takes, the latest 19 merges having added 1000 ms at the median and 22000 ms at the most.
2. Three tests of 12, 30 and 44 ms, 86 ms together, 0 % of the same.
3. "The host this work runs on gives no time of any test, so no test's time is known here."

## What each answered

1. (cost $0.0478, 99 % of its first request read from the cache.) It raised one point in `test_review`. It listed the
   five tests, each slower than 98 to 99 % of the project's tests, 21080 ms together, about 4 % of a merged check, and
   said that this is nearly the largest addition of the latest 19 merges. It said the five tests are all in
   `test_times`, so nothing is left to measure, but that each has one reading, so the milliseconds are rough. It asked,
   for each of the five, what it catches that no other test does, as a failing condition (which behaviour, removed,
   turns it red while the other four and the page's existing tests stay green), and why it cannot be smaller, faster or
   folded into another. It asked two things in particular: whether the five repeat the same start and render of the
   page and could share one start, be fewer or render less; and what each of the two slowest, 6080 and 5200 ms, catches
   that the other three do not.
2. (cost $0.0271.) It raised nothing. Its return was `{"outcome":{"dealt_with":[]}}`: no `test_review`, no `test_ok`,
   so no reason.
3. (cost $0.0189.) The same empty return.

## What this shows and what it does not

- Where the datum lists a batch of tests that take a large share of a merged check, T2's question 4 makes it raise one
  point that lists the tests and asks what each must show: shown by the first fork.
- Where the datum lists tests of milliseconds, or says the host gives no time, it raised no point: shown by the second
  and third forks.
- That it says why it raises none is not shown. The forks were asked to answer question 4 alone, in a form that requires
  no block; a real round of T2 must write `test_ok`, and T2's own words say to answer each question by its number in
  the why of `test_ok`, which is where the reason for not asking about time belongs. The forks did not use that block,
  so they say nothing of whether it is used.
- Three forks of one round are three readings, not a rate: how often a real round raises or leaves out such a point
  is not measured.
