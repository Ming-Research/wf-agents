# How long each job of ci.yml took, and where the Rust tests' time is, on 2026-11-22

Whom it serves: the decision to run the page's tests in a job of their own, and whoever next changes how the full
workflow is split. It is kept until the split is measured again.

## The question

Which job ends a run of the full workflow, and could the three Rust jobs be balanced by the time of their tests?

## Job times (measured on 2026-11-22)

Eight runs of `ci.yml`, six started by hand and two by a pull request, read with
`gh api repos/<repository>/actions/runs/<id>/jobs` (job `started_at` to `completed_at`, in seconds):

| Run | page | test 1 | test 2 | test 3 | lint | check |
|---|---:|---:|---:|---:|---:|---:|
| 45079169047 | 140 | 87 | 132 | 56 | 24 | 14 |
| 45078899020 | 149 | 115 | 136 | 77 | 27 | 9 |
| 45023725440 | 121 | 84 | 100 | 32 | 17 | 7 |
| 45022581594 | 110 | 71 | 92 | 38 | 20 | 7 |
| 45021408999 | 112 | 70 | 81 | 36 | 18 | 12 |
| 45021041042 | 126 | 79 | 113 | 29 | 27 | 17 |
| 45021043591 | 111 | 93 | 111 | 41 | 23 | 17 |
| 45023727656 | 103 | 70 | 94 | 37 | 10 | 8 |

The page job is the longest of the run in 6 of the 8, and within 15 s of the longest in the other two. Its steps, in two
of the runs: type-check 16 and 6 s, lint 7 and 10 s, the tests 81 and 75 s, the build 21 and 22 s.

## The Rust tests (measured on 2026-11-22)

The JUnit of the three Rust jobs of run 45079169047 (1969 tests, 560 s in all, each time as the job measured it while
three jobs shared the machine): 4 tests of 20 s or more hold 207 s, 10 of 10 s or more hold 293 s, 23 of 2 s or more
hold 357 s. The longest is `context::histories every_round_is_filled_with_what_its_session_lacks_by_the_kinds_of_its_cell`
at 101 s, in the job that took 132 s. A job that holds it cannot take less than that test, however the others are
divided, and nextest has no partition by time.
