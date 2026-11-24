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
