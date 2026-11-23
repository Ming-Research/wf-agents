# Does the full CI run sooner with `scope` and the page in two jobs: question and criteria

Whom it serves: item 92, which builds the job `scope` and the page's tests in a job of their own, and the review that
judges its result. It is kept until item 92's numbers are taken and `ci-job-split-measured.md` holds them; then it is
deleted.

This is written before any number is taken. A result that none of the rejections below meets keeps the part it is about.

Item 92 asks the owner to run the comparison before its change to `ci.yml` merges (a protected file, so the engine asks),
and records the numbers the owner gives and applies the criteria to them.

## The question

Does a `ci.yml` with the job `scope` (a wait every run pays, to leave the other jobs out of the 9% of runs that only
change `design/`) finish a full run on a commit that changes more than `design/` sooner than main's, and does it finish
sooner still with the page's tests in a job of their own?

## The comparison

- The two files compared: variant 1 is main's `ci.yml` with `scope` and a job for the design tree's check, the page in
  one job; variant 2 is variant 1 with the page in two jobs (its type-check, lint and build, and its tests).
- One commit, the same for every run: any commit that changes more than `design/`, pushed to a branch of the repository.
- At least 5 pairs, alternating: `gh workflow run ci.yml --ref main -f commit=<commit> -f call=<value>`, then the same
  with `--ref <the branch of item 92>` for variant 1, and then for variant 2, each pair's runs one after the other,
  never together. In half of the pairs another full run, or a run of the fast workflow, is under way on the machine.
- For each run, from `gh api repos/<repository>/actions/runs/<id>/jobs`: its wall (the run's creation to its last job's
  completion), and for each job its queue wait (`created_at` to `started_at`).
- One more run on a commit that changes only files under `design/`, and one that does and makes `design/lint.py` fail
  (a decision's first line over 200 characters): which jobs ran, the wall, and how `check` and `tree` ended.

## What rejects each part (a proposal; the owner may set others before anything is taken)

1. `scope` is rejected if the median wall of variant 1 is more than 10 s longer than main's on the
   same commit. Its break-even: about 9% of runs (19 of the 222 merges since 2026-11-01 change only `design/`) save about
   100 s, and the other 91% pay its cost, so a cost over about 10 s loses. Then each heavy job decides for itself in its
   checkout step, and the owner is asked again.
2. The page in two jobs is rejected if the median wall of variant 2 is not at least 10 s shorter than variant 1's, or if
   the median queue wait of any job is over 15 s (a check of variant 2 holds up to 7 of the 8 runners, where today it
   holds 5). Then variant 2's page split is dropped.
3. The design-only runs are rejected if a job other than `scope`, `tree` and `check` ran, if the wall is over 40 s, or if
   a failing `design/lint.py` does not make `tree` and `check` red.

## Where the numbers go

`research/ci-job-split-measured.md`, with the date they were taken; the decisions that rest on them cite that file.
