# Does the full CI run sooner with `scope` and the page in two jobs: question and criteria

Whom it serves: the owner, who runs the comparison below before the work merges, and the review that judges its
result. It is kept until the numbers are taken and `ci-job-split-measured.md` holds them; then it is deleted.

This is written before any number is taken. A result that none of the rejections below meets keeps the part it is about.

## The question

Does the branch's `ci.yml` finish a full run on a commit that changes more than `design/` sooner than main's, with the
job `scope` (a wait every run pays, to leave the other jobs out of the 9% of runs that only change `design/`) and, as a
second step, the page's tests in a job of their own?

## The comparison

- One commit, the same for every run: any commit that changes more than `design/`, pushed to a branch of the repository.
- Commit 1 of the work is the branch's `ci.yml` with `scope` and the tree's job. Commit 2 adds the page in two jobs.
- At least 5 pairs, alternating: `gh workflow run ci.yml --ref main -f commit=<commit> -f call=<value>`, then the same
  with `--ref <the work's branch>` at commit 1, and then at commit 2, each pair's runs one after the other, never
  together. In half of the pairs another full run, or a run of the fast workflow, is under way on the machine.
- For each run, from `gh api repos/<repository>/actions/runs/<id>/jobs`: its wall (the run's creation to its last job's
  completion), and for each job its queue wait (`created_at` to `started_at`).
- One more run on a commit that changes only files under `design/`, and one that does and makes `design/lint.py` fail
  (a decision's first line over 200 characters): which jobs ran, the wall, and how `check` and `tree` ended.

## What rejects each part (a proposal; the owner may set others before anything is taken)

1. `scope` is rejected if the median wall of the branch's file at commit 1 is more than 10 s longer than main's on the
   same commit. Its break-even: about 9% of runs (19 of the 222 merges since 2026-11-01 change only `design/`) save about
   100 s, and the other 91% pay its cost, so a cost over about 10 s loses. Then each heavy job decides for itself in its
   checkout step, and the owner is asked again.
2. The page in two jobs is rejected if the median wall at commit 2 is not at least 10 s shorter than at commit 1, or if
   the median queue wait of any job is over 15 s (a check at commit 2 holds up to 7 of the 8 runners, where today it
   holds 5). Then commit 2 is dropped.
3. The design-only runs are rejected if a job other than `scope`, `tree` and `check` ran, if the wall is over 40 s, or if
   a failing `design/lint.py` does not make `tree` and `check` red.

## Where the numbers go

`research/ci-job-split-measured.md`, with the date they were taken; the decisions that rest on them cite that file.
