# Which runs of ci.yml anyone reads, counted on 2026-11-22

Whom it serves: the decision that ci.yml runs only when the engine asks, and whoever next asks what CI costs. It is kept
until ci.yml's triggers or the engine's reading of runs change.

## The question

How many runs of `.github/workflows/ci.yml` did the engine's check read, and how many did nothing read?

## What reads a run (read from the code and GitHub's settings)

- The engine lists only runs started by hand: `crates/carry/src/host/github/gh.rs` asks for `event=workflow_dispatch`,
  and finds its run by the run name `ci <call>`.
- The repository's ruleset `protect main` holds only `deletion` and `non_fast_forward`, and the branch has no protection
  (`gh api repos/<repository>/rulesets`, `gh api repos/<repository>/branches/main/protection`), so no merge waits for a
  check of a pull request or a push.

## The counts (measured on 2026-11-22)

The latest 300 runs of ci.yml, 2026-11-19 to 2026-11-22, from
`gh run list --workflow ci.yml --limit 300 --json event,conclusion,createdAt,startedAt,updatedAt`. The wall time of a
run is `updatedAt - startedAt`; cancelled runs are left out of the minutes, not of the counts.

| Started by | Runs | Cancelled | Wall minutes of the others |
|---|---:|---:|---:|
| pull request | 204 | 47 | 370 |
| push to main | 44 | 0 | 89 |
| by hand (the engine) | 52 | 0 | 144 |

So 459 of 603 wall minutes (76%) were runs that nothing read. These are wall minutes of runs, not runner-seconds: a run
holds several runners at once.
