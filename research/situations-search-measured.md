# What the situations search and the tests' build profile cost, measured from 2026-11-24

Item #45. These are the numbers the work rests on for two choices of the owner: whether CI builds the crates `context`,
`engine`, `decide` and `work` at opt-level 3 (decision `wf-agents-tests-build-dependencies-optimized`), and how far
`wf-agents flow situations` can reach the lines real rounds stand in within 30 s (decision
`the-situations-tool-writes-the-situations-a-search-reached`). It serves the implementation of #45 and the reviews that
judge those two decisions. It is kept as long as the two decisions cite it, and replaced by the numbers of the change
that changes either.

Each claim says how it is known: (measured) on CI, with the run; (read) from source; (inferred) from measurements, not
tried.

## 1. What was asked, before anything was measured in this note

**Profile.** Does setting opt-level 3 for `context`, `engine`, `decide` and `work` in `[profile.ci]` save time in CI?
It is compared with the profile as it is (opt-level 0 for the workspace's own crates, 1 for `flow`), on one commit, in
one measure run: the file `measure/ci-test-build` builds the tests of those four crates with the profile `ci`, and
`measure/ci-test-build-optimized` does the same with `--config 'profile.ci.package.<crate>.opt-level=3'` for each of the
four. Both clean the four crates' artifacts first and share one dependency target directory kept for the run. The
proposal is rejected if the median compile time added by level 3 is not smaller than the test time it saves: the
engineer-flow test, 330.4 s in the profile as it is, against 62.7 s at level 3 on the first measurement (section 2).

**Search.** Is the curve of lines found by rounds flat at the bound of the tool (16000 rounds)? The search is compared
at 16000 rounds against what the independent histories stood in (random histories at three seeds and directed scripts,
sharing only `Sim` with the search). A second phase and cheaper rounds would be rejected if the lines the histories
stood in and the search did not find stayed at 293 or more of the engineer flow's lines, or the tool stayed over 30 s.

## 2. Measured before this note, commit 28f449828

| What | Value | Run |
|---|---|---|
| Tool, release, engineer flow, `measure/situations-search` candidate, after the first pair | 44.351, 44.384, 44.406, 44.861 s, median 44.406 s | measure run 45358285828 |
| Old tool on the same measurement (base) | 0.195 s (with a no-op build) | the same |
| Engineer-flow test, CI profile as it is | 330.401 s | fast run 45358287329 |
| Small-flow test, CI profile as it is | 32.670 s | the same |
| Engineer-flow test, `context`, `engine`, `decide`, `work` at opt-level 3 (scratch commit 28e3e748d) | 62.746 s | fast run 45358287221 |
| Small-flow test, the same | 4.512 s | the same |
| Test job, wall | 355 s as it is, 142 s at level 3 (the part of it that is not the engineer test: 25 s and 79 s, (inferred): the build and set-up of two single runs) | the same two |
| Lines found, engineer flow, 16000 rounds, 48706 states kept | 848; lines by rounds 1000 -> 625, 2000 -> 659, 4000 -> 708, 8000 -> 772, 12000 -> 795, 16000 -> 848 | fast run 45358287329 |
| Lines the histories stood in that the search did not find, engineer flow | 293: D1 6, D2 1, I1 30, P1 18, P2 25, R0 7, R1 5, T1 41, T2 53, T3 59, T4 48 | the same |
| Small flow | 117 lines found, 2 not found | the same |

The two profiles gave the same search result, line for line: the search is deterministic.

## 3. The compile time of the four crates at opt-level 3, commit e991eba08

One measure run, one request, both files on one commit (measure run 45377909715; `measure/ci-test-build` and
`measure/ci-test-build-optimized`, the files in section 8). Whole seconds of the compile of the tests of `context`,
`engine`, `decide` and `work`, their artifacts removed before each timing, the dependencies built once for the run.
Five pairs; each side's median, with its lowest and highest sample.

| Side | Profile as it is | Opt-level 3 for the four crates | Added by level 3 |
|---|---|---|---|
| candidate | 12.768 s (12.715 to 12.961) | 18.214 s (17.839 to 18.489) | 5.446 s |
| base | 12.641 s (12.552 to 16.034, the first pair) | 18.017 s (17.540 to 18.248) | 5.376 s |
| twin | 12.649 s (12.457 to 13.272) | 17.680 s (17.402 to 17.939) | 5.031 s |

(measured) What it says: the timed part is a compile of those crates, not a cache hit (inferred: the two files differ by
5 s on the same unchanged sources, which a build with nothing to do would not show; a no-op build was not timed). The noise, base against twin, is 0.01 s for the first file and 0.34 s
for the second; the first pair of the base is 3.5 s longer than its others, which is the build of everything the four
crates depend on that is not one of them (it is not in any median).

The result against the criterion of section 1: level 3 adds 5.4 s of compile (5.0 to 5.4 s over the three sides), and
saves 267.7 s of the engineer-flow test on the first measurement (330.401 s as it is, 62.746 s at level 3, section 2,
the search then at 16000 rounds). The proposal is not rejected: the saving is about fifty times what it costs. The
estimate of the first round, 54 s more to compile, came from two single runs of whole jobs, whose set-up differed
(inferred), and is replaced by this.

## 4. The search and the tool's time, by commit

(measured) The tool is `measure/situations-search` on the engineer flow in a release build, whole seconds: the median
of the candidate after its first pair, which holds the build, and the base's median (the old tool, which is a no-op
build and 0.19 s) shows what the build part is. A run that overlapped other CI jobs reads high; the samples are in the
run, and the table says where one did.

| Commit | What it changed in the search | Rounds | Lines found, not found | Tool, median | Engineer test, CI profile as it is | Runs |
|---|---|---|---|---|---|---|
| 28f449828 | the first search | 16000 | 848, 293 | 44.406 s | 330.401 s | measure 45358285828, fast 45358287329 |
| 3ac4514ef | a return that hands in only part of what is written beside a waiting question, a key that does not build the message | 16000 | 864, 297 | 37.862 s | 294.297 s | measure 45363680265, fast 45363681507 |
| c6e281759 | the rank of a kept state is the picks its cell has had in all, not the rounds it carried (the two cells that only the engine runs carried none, so ranked lowest and were picked about 10000 times) | 16000 | 1224, 57 | 28.238 s (cfa47e4fb, the same search, measure 45371301597) | 246.523 s | fast 45369195945 |
| bec3864a0 | what the manager reads from the records once for an entry, not once for each datum | 16000 | 1224, 57 (the same) | 18.754 s | not run | measure 45373319393 |
| 0590f5bd9 | the bound raised to 24000 rounds | 24000 | 1259, 41 | 31.515 s | 266.066 s | measure 45374744238, fast 45374745241 |
| c2f830af1 | where each round began and ended, and the runs of a line, read once for an entry | 24000 | 1259, 41 (the same) | 32.118 s | 208.054 s | measure 45376367414, fast 45376368853 |
| e991eba08 | the bound lowered to 22000 rounds | 22000 | 1255 at the bound (the curve below), not measured | 28.437 s (28.381 to 28.457) | not run | measure 45377909715 |

The run of c6e281759 itself read 53.1 s because it overlapped other jobs (samples 120.6, 53.1, 63.0, 33.0, 28.3 s,
measure 45369195228); the run of its search on cfa47e4fb, which reverts a try on top of it and changes only a document,
is the clean one. The small-flow test passes at c6e281759 and after it, with 0 lines not found; it takes 33.5 s to 44.2 s
in the CI profile as it is (fast runs 45376368853, 45374745241, 45373320435).

Where the 16000 rounds' time went at c6e281759, in the CI profile as it is: `key` 52.3 s, `enter` 50.5 s, `engine`
7.8 s, `clone` 2.4 s, `note` 0.46 s, `befores` 0.25 s, `entry` 0.10 s. At 24000 rounds on c2f830af1: `key` 31.3 s,
`enter` 30.6 s, `engine` 13.3 s, `clone` 3.8 s, `note` 0.64 s, `befores` 0.41 s, `entry` 0.17 s.

## 5. Lines found by rounds, 24000 rounds, commit 0590f5bd9 (the search is deterministic; c2f830af1 gives the same)

(measured, fast run 45374745241) 1000 -> 840, 2000 -> 943, 3000 -> 1002, 4000 -> 1054, 5000 -> 1091, 6000 -> 1121,
7000 -> 1141, 8000 -> 1152, 9000 -> 1164, 10000 -> 1170, 11000 -> 1180, 12000 -> 1185, 13000 -> 1196, 14000 -> 1218,
15000 -> 1221, 16000 -> 1224, 17000 -> 1229, 18000 -> 1229, 19000 -> 1233, 20000 -> 1239, 21000 -> 1246,
22000 -> 1255, 23000 -> 1258, 24000 -> 1259.

The last 8000 rounds add 35 lines, 3 per cent of what was found by 16000. Rounds of each cell at 24000: D1 3657, D2 1603,
I1 2920, P1 2615, P2 1624, R0 1782, R1 2577, R2 1458, T1 1546, T2 1400, T3 1474, T4 1344.

## 6. What was tried and left out

(measured, each in its own fast run)
- Rollouts of up to 24 steps from a kept state (bf937ff0a, run 45351077614): 804 lines, 376 not found, 117.7 s.
- Skipping a state whose key was already kept (3bc96995f, run 45356179634): 869 lines, 239 not found, 101.8 s.
- A finer key with progress counters for each cell and each datum (2c19a8d35, run 45365109509): 865 lines, 263 not found,
  115.1 s, 85875 states kept in 14563 rounds.
- Three of the ways a round returns tried in turn each time a state is gone on from (4436a6e12, run 45370602895): 1165
  lines, 86 not found.
- Variants of a return that leaves unmatched all but one of what ran the round (c7dccabed, run 45372461896): 1186 lines,
  87 not found.

The fix that mattered was the rank (c6e281759: 297 not found fell to 57); every later try that changed what the search
chooses moved the number by tens of lines either way, which is the size of the difference between two seeds, so none of
those is a result.

## 7. What is not found at 24000 rounds, commit c2f830af1

(measured, fast run 45376368853, and the same at 0590f5bd9) 41 lines that the independent histories stood in and the
search did not find: D1 1, I1 3, P1 1, P2 2, T1 4, T2 14, T3 10, T4 6.

- T2 (14) and T3 (10), 24 lines, are two situations: the first round of a cell, entered after a hand-in, with every
  datum it is given held at the version the cell last worked from while a later one exists (`holds_last`; read from
  `crates/context/src/manager.rs`). Each is a session that was kept while the acceptance, the background, the code, the
  decisions, the evidence, the title, the tree entries and both plans were written again, a long chain of other cells'
  rounds (inferred from that definition; no history was read to confirm it).
- The rest are `holds_latest` or `holds_last` of the questions blocks (`d1_questions`, `i1_questions`, `r1_questions`) of
  D1, I1, P1, P2, T1, and `holds_latest` of five blocks in a second round of T4.
- All are lines of one or two situations each; there are about ten situations in all, deeper than the search reaches by
  its choices of actions in the rounds it has.

## 8. The content of the two measurement files, as they were run on e991eba08

`measure/ci-test-build`:

```bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/ci-test-build-target"
cargo clean --profile ci -p context -p engine -p decide -p work
cargo test --no-run --locked --profile ci "$@" -p context -p engine -p decide -p work
```

`measure/ci-test-build-optimized`:

```bash
set -euo pipefail
exec bash "$(dirname "$0")/ci-test-build" \
  --config 'profile.ci.package.context.opt-level=3' \
  --config 'profile.ci.package.engine.opt-level=3' \
  --config 'profile.ci.package.decide.opt-level=3' \
  --config 'profile.ci.package.work.opt-level=3'
```

They were run with `run_tests` naming both in `measure`, and are deleted now that the profile is changed (section 3).
Their medians can be had again by writing them out and running them in the root of a checkout in the order above.
