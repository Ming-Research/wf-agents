# The times of the tests

How the engine judges how long the project's tests and jobs take, from what its CI measured, what it tells the reviewers, and the budget a fast run keeps to.

### decision the-engine-judges-no-new-tests-time
The engine judges no new test's time: it gives the test reviewer what the CI measured as the datum `test_times`, and the reviewer decides whether a question about time is worth asking.
- rejected: an agent proposed that the engine list the new tests at or above the time where the slowest tests hold half of the suite's time; the owner rejected it, because a batch of tests of 2 to 6 s each is never listed, nor is a creep by smaller ones (data: `research/test-times-measured.md`).

### decision the-ci-grows-by-medians-of-its-merged-checks
The CI has grown where the latest merged checks of a job's time, or of the tests' time in all, are above the median of as many before them more often than chance allows; its merge is named.
- rejected: an agent proposed judging a change's added tests and each job's time by the existing line of the history; the owner rejected it, because a change must add about twice the largest batch merged to be listed, so steps of that size never show (data: `research/test-times-measured.md`).

### decision a-fast-run-keeps-to-a-budget
A call of `run_tests` has a budget, the project's `ci.fast.budget` or else `run_tests.toml`'s; its fast run stops the tests at it, and what it stopped is said apart from failures and never run again.
- replaced: A fast run bounded only by its workflow's `too_long` and each test's stop was on main; the owner changed it, because fast checks ran ten minutes and more, mostly in tests named whole, and nothing held a run to the time an agent waits for its result.

### decision a-request-known-to-run-over-its-budget-runs-nothing
A `run_tests` request whose tests' medians in the project's earlier fast runs, one alone or together, exceed its budget runs nothing and says why per test; an untimed test is never refused.
- replaced: Running every request, whatever its tests took in earlier runs, was on main; the owner changed it, because a run known to go past the budget would only be stopped there, its machine time spent for no result.
