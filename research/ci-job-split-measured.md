# What a run of the full CI costs with `scope`, `tree` and the page's tests in a job of their own

Whom it serves: the decisions of `design/decisions/testing.md` and `design/decisions/design-tree.md` on the jobs of
`ci.yml`, which cite it, and whoever next changes how the full CI is split. It is kept until the numbers are measured
again after the next change to the workflow's jobs.

Each claim says how it is known: (counted) from the repository, (read) from the code or the workflow files, (measured)
by the runs below.

## How often a change touches only the design tree (counted on 2026-11-23)

Of the 247 first-parent commits of main from 2026-11-01 to `c27a01de8` that changed a file, 21 (8.5%) changed only files
under `design/`. Counted with, for each commit `$c` of
`git log --first-parent --since=2026-11-01 --format=%H c27a01de8`, the files of `git diff --name-only $c^1 $c`: the
commits with no parent or no changed file are left out, and a commit counts when no file is outside `design/`.

## What reads the tree's real files (read on 2026-11-23)

No test of the Rust crates or of the page reads a file of the repository's own design tree. A search of `crates/` for
`design/` finds only paths in made data: trees the tests make in a temporary directory, text of fixtures and strings,
and a project declaration that is a fixture (`crates/flow/tests/project/project.toml`); `client/` has no `design/` at
all, and `crates/web/build.rs` and the manifests do not name `design`. The tests that read the repository's own files
read `flow/` and `.github/workflows/` (a search of `crates/` for `CARGO_MANIFEST_DIR` and `include_str!`), which a
change of only `design/` does not touch. `design/lint.py`, `design/lint_test.py`, `docs/check_citations.py` and
`docs/check_citations_test.py` are run by the job `tree` of `ci.yml`, and the first and third also by the lint of
`fast.yml`; no test runs them.


## What a full run of main's `ci.yml` takes (measured on 2026-11-23)

Eight successful runs of main's file that the engine asked for, the last eight before 15:40 UTC, got with
`gh run list --workflow ci.yml --event workflow_dispatch --branch main` and, for each run, `gh api
repos/<repository>/actions/runs/<id>` (wall: creation to last update) and `.../runs/<id>/jobs` (each job from `started_at`
to `completed_at`). Seconds. Page is the one job `page`; the three Rust jobs are `test 1`, `test 2` and `test 3 of 3`.

| Run | Wall | page | test 1 | test 2 | test 3 | lint | Longest job |
|---|---|---|---|---|---|---|---|
| 45207563003 | 129 | 99 | 84 | 103 | 65 | 33 | test 2 |
| 45205075023 | 120 | 101 | 81 | 97 | 56 | 30 | page |
| 45202751163 | 152 | 107 | 116 | 137 | 92 | 66 | test 2 |
| 45202215743 | 142 | 121 | 88 | 104 | 55 | 31 | page |
| 45200432909 | 128 | 103 | 89 | 106 | 62 | 37 | test 2 |
| 45199905952 | 135 | 111 | 67 | 106 | 49 | 45 | page |
| 45199400513 | 137 | 109 | 94 | 115 | 64 | 34 | test 2 |
| 45198931733 | 121 | 101 | 80 | 106 | 62 | 24 | test 2 |

The median wall is 132 s (120 to 152). The page job takes 99 to 121 s (median 105), and is the longest job in 3 of the 8
runs, by 4, 5 and 17 s over the next one (`test 2 of 3` in each); in the other 5 `test 2 of 3` is the longest. So the
most that taking work out of `page` can shorten a run is those 4, 5 and 17 s in 3 of 8 runs, 3 s on average, and none
in the 5 others; how much of `page` its tests are (the plan took about 75 to 81 s) and what two jobs on two runners cost
in set-up are not measured. Every job waited 1 to 3 s for a runner.

The slowest test of five of those runs (the first, and 45205075023, 45202751163, 45202215743, 45200432909), read from
each run's artifact `test-results` (`gh run download <id> -n test-results`, the `time` of each `testcase`), took 57,
68, 60, 68 and 62 s: always `every_round_is_filled_with_what_its_session_lacks_by_the_kinds_of_its_cell`. The next
one took 36 to 50 s. A split of the Rust jobs by the tests' time is then limited by `test 2 of 3` at 97 to 137 s, of which
the longest test is 57 to 68 s; no test took 101 s in any of them.

## What is not measured

The owner chose to merge without a comparison run by hand of the branch's workflow against main's, so no number of the
branch's workflow is taken here. These are therefore inferred, not measured:

- what `scope` adds to a run that changes code (a job on GitHub's machine that makes one API request and ends in seconds,
  which `page`, `page tests`, `lint` and `test` then wait for);
- how long a run of only the design tree takes (it runs `scope`, `tree` and `check`, none of which builds anything);
- whether `scope` says the right thing for the commit the engine gives, a merge of main into the work's branch.

Where they first show: the work's own pull request into main runs the branch's file (`on: pull_request`), so the jobs of
that run, which changes code, show what `scope` adds and each job's wait for a runner; a run of only the design tree, and
`scope` on the engine's own request, show on main after the merge (`gh api repos/<repository>/actions/runs/<id>/jobs`, the
jobs' times). The owner reads them there.
