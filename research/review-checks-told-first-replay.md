# What a review checks, told first: R0, R1, I1 and T2 replayed

For: the owner, who approves the change to cells R0, R1, I1, T2 and T3 at the merge, and its reviewer.
Kept: while the lines replayed here stand as written; deleted when one of them next changes, git keeping it.
Written: 2026-11-06, on the branch `review-checks-told-first`, from the trial database as it stood that day.

## 1. Question

The cell that does the work is told what its review will check, so that the review finds only what was told and not
done. Three problems are met that way, and each change was replayed on past works, old prompt beside new:

- Performance (#316: W368 merged #312's indexes, and a read went from 106.8 to 5,596 ms at ten times the data). I1 is
  told to measure where the change can alter how much runs, and T2 to raise a measurement missing there, or a slowdown
  measured, as a finding. Does I1 on W368 measure, and does T2 on W368's own head raise the missing measurement?
- A defect's class and its prevention (#306 pinned git's hooks, #309 then the other programs git runs, W344 found four
  more). R1's `cause` asks, for a defect, for its like cases and what keeps the kind from coming back, which the change
  covers or which go to side reports; T3 checks the change covers what the cause names. Does R1 on W339 name the other
  programs git runs, and what does its change do with them?
- R0's purpose: it gathers history so that the work learns its lessons and does not take again a path that failed.
  Does R0's brief on W321, W324, W340 and two ordinary works (W350, W356) name the lessons?

Besides, R1's line on probes that showed nothing was replayed in a new wording on W349 (section 4.4).

## 2. What changed in the messages

- R0: its task becomes "This round gathers the history of this item, so that its work learns what was learned before
  and does not take again a path that failed: ... the approaches dropped, with why; the decisions overturned, with what
  replaced them, the design tree's own history among them; ... The design tree as it stands the work reads itself; the
  history no one looks back at unasked, and this round brings it. Give the address of each with the lesson it holds,
  ...". The part on `decisions/` is as it was. Its brief is sectioned by lessons first: paths that failed or were
  dropped, decisions overturned.
- R1: the description of `cause` says, for a defect, its root and what keeps its kind from coming back, and its like
  problems, the same mistake or the same kind of hole: the change covers them, and each this work does not take goes
  into side reports, saying why. The description of the change says it covers them, as the cause names them.
- I1: for a work that delivers a change, "Where the change can alter how much runs per event, round, work or request,
  or how that grows with the data, measure it with measure, before and after the change, at the size the data has and
  at a larger one, and hand in the numbers in measured", and a field `measured` (what, its run, the numbers).
- T2: "where the change can alter how much runs ..., was it measured: a measurement missing there, or a slowdown
  measured, is a finding", and the input of what I1 measured.
- T3 (not replayed, section 5): "for a defect, whether the change covers the like cases and what keeps it from coming
  back, as the research's cause names them".

## 3. How the rounds were run

As in [r1-defect-replay.md](r1-defect-replay.md): each round rebuilt from `rounds.input`, the recorded command line run
in a scratch clone at the round's commit with later refs removed, a new session, no hooks, scratch directories in place
of the trial's. Two things are new here. Each case reads a copy of the trial database with every change after the
original round undone from its `changes` table, and rounds, tool runs and changes after it deleted; the message's
database path points to the copy, so R0's direct reads and the queries run by hand see only what the original saw.
I1 on W368 ran in a fresh session (the original went on in R1's session, which lives in the engine's data directory);
its message gives R1's understanding, plan and acceptance from the record, as the engine gives them to a fresh I1
session. The `ci` and `ci_before` runs the rounds asked for ran through the fast CI on branches named `replay/...`.
Cost: about 10 USD.

## 4. Cases

### 4.1 R0 on five works

| Work | Original and old prompt | New prompt |
| --- | --- | --- |
| W321 (#286, check the format before ci) | Original: "#109 allowed `cargo fmt`", missing that #266 (4110a676) refused it again; R1 found that in the code, and the work went to deep research. The old prompt today found #266. | Found #266 too, under "paths turned down: why one line in AGENTS.md is not enough", with the earlier refusal of `npm --version` and the owner's past ruling that permissions are the owner's to change |
| W324 (#290, do not close a merged PR) | #267's check before merging, observation 67, listed as related | The same, with the lesson written out: the PR's state is read when the action runs, not when it is queued; says no dropped approach is recorded |
| W340 (#307, record the engine's starts and stops) | Related items, the owner's #211 decision on retries | Also matter 811, where the owner said the 9 to 10 minute gaps were the engine stopped for an install, against the item's premise that their cause is unknown; the existing global offline stop (88e4ca20); the owner's #32 rule of stops by cause |
| W356 (#223, quote a chat bubble) | Draft layer, the owner's decision, the draft designs dropped in #222 | Same ground in fewer words, lessons first ("what of the kind is already merged"), and the owner's word in matter 437 that the item's premise of long chats is to be checked |
| W350 (#311, stale ci_before results) | Same returns sent back, related items, owner decisions | A section of paths dropped or replaced (matter 600's correction of how ci_before waits; #288 dropped for the 14900K fast CI); the CLI refused its return for its form four times, so the brief is read from its last full attempt |

Judgment: the new briefs put the lessons first and say them, where the old ones list addresses; W340's brings a fact the
work needed and the old missed. W321's lesson the old prompt found too on this day, so that case shows no difference in
what was found, only in how it is put.

### 4.2 R1 on W339 (#306, pin git's hooks in the engine)

| Run | Like cases | Prevention | Side reports |
| --- | --- | --- | --- |
| Original R339-5 | `core.sshCommand`, `core.fsmonitor`, merge and filter drivers, `credential.helper`: "not in this item's scope" | a constant of pinned settings, "one line to add one" | yes |
| Old prompt | `core.sshCommand`, `core.fsmonitor` | the setting in a constant | yes |
| New, first wording | `core.sshCommand`, `core.fsmonitor`, "not handled in this item" | the setting in a constant | none |
| New prompt | `core.sshCommand`, `core.fsmonitor`, `filter.*`, not taken, since pinning `core.sshCommand` may break the owner's own SSH; the owner's choice of the agent's boundary decides | one entry for the engine's git, used by every site, and a test that fails when production code builds a `git` command elsewhere | yes, saying why |

With the first wording (like cases "which the change covers"), R1 reproduced the hooks with three runs of `ci_before`
(fast CI runs 42344499159, 42344657630, 42344807464) and named the like cases without taking them or reporting them.
The wording in the change, rerun from the start, took the item's own run on a hosted runner as the defect seen, named
the kind, wrote what keeps it from coming back and covered every site of the engine's git, and sent the other programs
to side reports with why. It still does not cover them in the change: its reason, a conflict with the owner's SSH
settings, is one the owner rules on.

### 4.3 I1 and T2 on W368 (#312, index the reads that scan a table)

| Run | Decision |
| --- | --- |
| Original R368-12 to 20 | plan tests only, through `ci`; no `measure`; handed in done |
| Old prompt | its first round the same: `ci` with seven plan tests |
| New prompt, rounds 1 and 2 | `ci` with seven plan tests, then `ci_before` (runs 42344705099, 42345174003, 42345191612) |
| New prompt, round 3 | `measure` of the next-transaction read (E2 of the report) at 1 and 10 times the data, against the code before the change, with what passes and what overturns written first |
| T2, old prompt, on the original head | one finding: a read still scans `sessions` |
| T2, new prompt, on the same head | the same, and first: "no measurement on the branch after the change: the implementation ran no measure" |

The measurement (45 minutes on a hosted runner) was not run; the decision to measure is the change. I1 measured the
read it meant to speed up, not the reads whose plans the new indexes change, where #316's slowdown was; T2's new line is
what would raise that.

### 4.4 R1's probe line on W349

Within the round bound, the line "a probe there that showed nothing is taken out once run" held in one run of two
(the PR before this one). With "is taken out in the round that sees its result, which is noted in `research/`", two
runs of two took the probes out and wrote the note (fast CI run 42344422210 on `replay/w349-probe-clause`: all eight
tests passed, nothing reproduced), and both asked the owner.

## 5. Not replayed

T3's line checks the change against what R1's cause names; no past return names like cases the change was to cover,
so a replay would check nothing. The line is the counterpart of R1's description, as T2's is of I1's.

## 6. Conclusion

I1 now measures a change that alters how much runs, and T2 raises a missing measurement as a finding on the head W368
merged without one. R1 names a defect's kind and what keeps it from coming back, and reports the like cases it does not
take, saying why; whether it takes them is still its judgment. R0 writes the lessons of the history first. The probe
line holds in a slightly different wording, which this change carries.
