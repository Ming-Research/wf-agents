# What reading the design tree takes on each line, measured on 2026-11-22

Item #30. Whom it serves: the owner, who chooses whether the lines of a work are given the entries of the tree that bear
on its item from one reading, and whether a reading is reused across works; and the reviews of that work. It is kept
until the reading is measured again after that change is merged (section 5 says how), and goes then. It changes no
engine or flow file.

Files, in `research/`: this report; `tree-reading-measured.sql`, the queries on the engine's records; and
`tree-reading-stream.py`, which measures what one round read of the tree from its stream in the data directory
(`rounds/<work>/<run>-<round>.stream.jsonl`, which the store does not hold), as research/i1-round-cost-measured.md read
the streams. Each claim says how it is known: (measured) on that day, (read) from a record or a file, (inferred) from
measurements, not tried. No rejection criterion was written before these measurements: they are exploratory, and they
count what happened; none tests a proposal.

## 1. The tree

On main at 5c3e0f71e (read): 88 principles and 396 decisions, 484 entries of 302,690 bytes, 625 bytes on average, in
310,677 bytes of files. The constitution and the tree's rules are in every session's system prompt already
(`flow/agents/session.toml`). The tree was rebuilt in #377 on 2026-11-21; works begun before that read the old tree.

## 2. How much of the tree each line's first round reads

The bytes of the tree's tool results in the first round of a run (measured, `tree-reading-stream.py` on the streams;
the cost of that round from Q1). R0 and D1 do only that reading in their first round; P1's and T1's first rounds read
the tree for their line and also do their own review, so their cost is not the tree's alone.

| Line, cell | Rounds | Files of the tree read | KB of the tree read | Cost of the round |
|---|---|---|---|---|
| engineer, R0, first run | W22, W23, W24, W26, W28, W29, W30 | 17 to 27 | 87.0 to 158.2 | $0.24 to $0.34 on Sonnet 5.5; $0.69 on Opus 5.5 (W26, W30) |
| engineer, R0, run after a new session | W21 runs 42 and 80, W28 run 25 | 18 to 30 | 145.1 to 191.3 | $0.47 to $1.08 |
| deep_researcher, D1 | W21 run 14, W30 run 10 | 15, 27 | 129.6, 163.6 | $1.56, $1.83 (Opus) |
| plan_reviewer, P1 | W20, W21, W22, W23, W26, W28, W29 | 1 to 19 | 12.6 to 107.7 | $0.27 to $3.73 (Opus) |
| code_reviewer, T1 | W20 run 88, W21 run 109, W23 run 26, W29 run 60 | 0 | 0 to 8.0 (searches only) | $0.20 to $4.09 |
| code_reviewer, T2 | W20 run 97 | 0 | 0.8 | $2.16 |
| code_reviewer, T3 | W29 run 70 | 4 | 33.9 | $0.11 (Sonnet) |
| tester, T4 | none read it | | | |

So the lines do not each read the whole tree. The engineer and the deep research read a third to a half of it; the
plan review reads up to a third and once read one file (W29 run 26); the code review reads almost none at T1 and T2, and
at T3 only the files the change touches, as its task says. What the plan review and the code review see of the tree
is not what the engineer worked from.

## 3. What the reading after a new session costs

R0 watches `engineer_session` and reads the tree again in each new session of the engineer line. Its runs after the
first, in the works of Q2, are as many as that line's new sessions in Q3 (measured): W21 7 runs, $8.09 (its session
was compacted 8 times); W28 2 runs, $6.63 (run 22, on Sonnet, cost $6.16 for 6 calls at 180 to 209 K of context);
W23 2, $0.56; W26 1, $1.04; W20 1, $0.78; W11 1, $0.19; W12 1, $0.11. Together $17.40, 4.4% of the $391 these works
cost (Q2). All R0 runs together are $22.47, 5.7%.

## 4. Does the reading at the start find what the reviews later needed

The oracle: the entries that the plan's tree review (P2, `tree_review`) and the change's tree review (T3,
`design_review`) named in their points, in the works on the rebuilt tree (Q6), each placed in its file by `git log -S`
on its heading. Against them, the files of the tree the work's first R0 read (`--files`).

| Work | Entries named | In a file R0 read |
|---|---|---|
| W22 | 6 principles (lean-agent-input, measure-speed-and-cost, nothing-passes-for-normal, solve-within-the-design-first, generalise-at-the-second-use, one-thing-one-place) | 6 |
| W23 | decision the-repository-is-worked-in-one-act-at-a-time | 1 |
| W26 | 4 principles of `principles/frontend.md`, 2 decisions of `decisions/frontend.md` | 0 |
| W28 | principle nothing-passes-for-normal, decision a-return-says-what-it-dealt-with | 2 |
| W65 | decision what-went-past-its-line-is-given-in-data-only | 1 |

10 of 16 (measured). Left out: W21, begun on the old tree and read again by R0 eight times across the rebuild, so that
which reading a point came after is not clear; and W85's one named entry, permissions-are-rules-not-a-mode, which W85's own
plan wrote (read, `git log -S`). one-thing-one-place was in `principles/common.md` until #385 took it out (read). The 6 missed are all in W26, whose item (#26, rounds cut off or ended without waste) names no
part of the page, while its plan reached the page. There the plan review found them by reading further: W26's P1
(run 19) read `decisions/frontend.md`, and its P2 (run 22) read `principles/frontend.md` (measured). So what is read at
the start cannot know where the plan will reach, and the reviews' own further reading is what found the rest
(inferred from one work). At entry level, which entries a reading would name, nothing was measured: no selection was
run.

## 5. Whether a reading could be reused by the next work

The tree on main changed in 24 of the 31 merges since 2026-11-19, and in 55 of 71 since 2026-11-15 (read, `git log
--first-parent` of main on `design/principles`, `design/decisions`, the constitution and the README). But the owner
starts works in batches: of the 32 works whose first R0 run is in Q5, 24 began it on a git tree of the design tree
that another work's R0 had begun on less than an hour before, 15 of them in one batch of 16 works let on within 20
seconds of each other, whose first R0 runs began within 5 minutes (measured, Q4 and Q5). So a reading made once per version of the tree would have served about three
works in four.

What it would save, and what it rests on (inferred): a first R0 round costs about $0.30 on Sonnet and $0.70 on Opus,
1% to 3% of most works; a fork that reads a session already holding the tree from a warm cache would cost a few cents.
It would hold only if the forks' requests begin with the same bytes as the session they fork. They do not today: each
work's R0 runs in that work's worktree, and the program's system prompt names the working directory and the date
(read in this session's own system prompt), so two works' sessions share no cached start. A kept session would have to
run in a directory of its own and hold the whole tree, which is what decision the-tree-is-read-in-a-cell-of-its-own
turned down for a line's session, because a larger project's tree overflows it. Not checked: whether `--fork-session`
works from another directory or account, and whether a session holding the whole tree names the entries an item needs
as well as R0's reading for that item does.

## 6. How to measure again

After the change, take the first rounds of the cells that read the tree in the next works, as section 2 does, and the
runs of R0 after a new session, as section 3 does: section 3's runs should be gone, and the reviews' first rounds should
read only what they read beyond the entries they were given. Count the size of each work's list of entries as it is
given (the `given` records).
