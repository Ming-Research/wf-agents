# Which runs of ci.yml anyone reads, counted on 2026-11-23

Whom it serves: the decision that ci.yml runs only when asked for by hand or on a pull request that is not a work's, and
whoever next asks what CI costs. It is kept until ci.yml's triggers or the engine's reading of runs change.

## The question

How many runs of `.github/workflows/ci.yml` did something read, and how many did nothing read?

## What reads a run

- The engine's check lists only runs started by hand: `crates/carry/src/host/github/gh.rs` asks for
  `event=workflow_dispatch`, and finds its run by the run name `ci <call>` (read from the code).
- The repository's ruleset `protect main` holds only `deletion` and `non_fast_forward`, and the branch has no protection
  (`gh api repos/<repository>/rulesets`, `gh api repos/<repository>/branches/main/protection`), so no merge waits for a
  check of a pull request or a push (read from GitHub's settings).
- A pull request that the engine opens for a work is on the branch `work-<name>` (`crates/carry/src/git.rs`), and the
  engine reads the run it asked for by hand on the same commit, not the pull request's. A pull request opened in a
  session of the owner's, on a branch that does not start with `work-`, is one whose result the owner reads before it
  merges (the owner's account; 19 of the latest 40 runs were of such pull requests).
- Nothing reads a push to main.

## The counts (measured on 2026-11-23)

The latest 300 runs of ci.yml, 2026-11-20 to 2026-11-23, from
`gh run list --workflow ci.yml --limit 300 --json event,headBranch,conclusion,createdAt,startedAt,updatedAt`. The wall
time of a run is `updatedAt - startedAt`; cancelled runs are left out of the minutes, not of the counts.

| Started by | Runs | Cancelled | Wall minutes of the others | Read by |
|---|---:|---:|---:|---|
| pull request on a branch of a work (`work-…`) | 101 | 31 | 202 | nothing |
| push to main | 54 | 2 | 114 | nothing |
| pull request on another branch | 85 | 6 | 166 | the owner |
| by hand (the engine) | 60 | 0 | 175 | the engine |

So 316 of 657 wall minutes (48%) were runs that nothing read. These are wall minutes of runs, not runner-seconds: a run
holds several runners at once.
