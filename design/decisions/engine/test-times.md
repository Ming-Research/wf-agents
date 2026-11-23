# The times of the tests

How the engine judges how long the project's tests and jobs take, from what its CI measured, and what it tells the reviewers.

### decision the-engine-judges-no-new-tests-time
The engine judges no new test's time: it gives the test reviewer what the CI measured as the datum `test_times`, and the reviewer decides whether a question about time is worth asking.
- rejected: an agent proposed that the engine list the new tests at or above the time where the slowest tests hold half of the suite's time; the owner rejected it, because a batch of tests of 2 to 6 s is never listed, and a creep by tests under that time is never asked about.

### decision the-ci-grows-by-medians-of-its-merged-checks
The CI has grown where the latest merged checks of a job's time, or of the tests' time in all, are above the median of as many before them more often than chance allows; its merge is named.
- rejected: an agent proposed judging a change's added tests and each job's time by the existing line of the history; the owner rejected it, because a change must then add about twice the largest batch ever merged to be listed, and a CI that grows by steps of that size is never flagged.
