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

No test of the Rust crates or of the page reads a file of the repository's own design tree. A search of `crates/` and
`client/` for `design/` finds only trees the tests make in a temporary directory, and `crates/web/build.rs` and the
manifests do not name `design`. `design/lint.py`, `design/lint_test.py`, `docs/check_citations.py` and
`docs/check_citations_test.py` are run by the job `tree` of `ci.yml`, and the first and third also by the lint of
`fast.yml`; no test runs them.

## The comparison and what rejects each part

Whom the comparison serves: the owner, who runs it, and the review that applies the criteria. It is written before any
number is taken. A part that a criterion rejects is dropped, and its decision then says what the numbers were.

Two variants of `ci.yml`, as the work's commits hold them, against main's file:

- variant 1: `scope` and `tree` added and the jobs of the code made to wait for `scope`, the page still in one job;
- variant 2: variant 1 with the page's type-check, lint and build in `page` and its tests in `page tests`.

The runs, by hand: `gh workflow run ci.yml --ref <ref> -f commit=<commit> -f call=<value>`, with `<ref>` a branch or tag
at the file of main, of variant 1, or of variant 2, and `<commit>` one commit that changes a file outside `design/`,
pushed to a branch of the repository, the same for every run. At least 5 runs of each, in turn, one run after the other,
and in half of them another run of the full or the fast workflow is under way on the machine. For each run, from
`gh api repos/<repository>/actions/runs/<id>/jobs`: its wall time (the run's creation to the last job's completion), and
for each job its time from start to end and its wait for a runner (`created_at` to `started_at`). The ten slowest tests
are in the summary of each run of main's file. One run of variant 2 on a commit that changes only files under
`design/`, and one on such a commit that makes `design/lint.py` fail (a decision's first line over 200 characters).

1. `scope` is rejected if the median wall time of variant 1 is more than 10 s longer than main's. Its break-even: 8.5%
   of the runs save about 100 s and the other 91.5% wait for it, so a cost over about 10 s loses. Then each job of the
   code decides for itself in its first steps, and the owner is asked again.
2. The page in two jobs is rejected if the median wall time of variant 2 is not at least 10 s shorter than variant 1's,
   or if the median wait for a runner of any job is over 15 s (a run holds up to 6 of the eight runners). Then the page
   is one job again.
3. A run on a change of only the tree is rejected if a job other than `scope`, `tree` and `check` ran, if its wall time
   is over 40 s, or if the failing `design/lint.py` does not make `tree` and `check` red.
