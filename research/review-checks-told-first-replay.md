# What a review checks, told first: R0, R1, I1, T2 and T3 replayed

For: the owner, who approves the change to cells R0, R1, I1, T2 and T3 at the merge, and its reviewer.
Kept: while the lines replayed here stand as written; deleted when one of them next changes, git keeping it.
Written: 2026-11-06, on the branch `review-checks-told-first`, from the trial database as it stood that day.

## 1. Question

The cell that does the work is told what its review will check, so that the review finds only what was told and not
done. Three problems are met that way, and each change was replayed on past works, old prompt beside new:

- Performance (#316: W368 merged #312's indexes, and a read whose plan the new index changed went from 106.8 to
  5,596 ms at ten times the data). I1 is told to measure what the change can make slower as well as what it is for, and
  T2 to raise a measurement missing there, or a slowdown measured that was not accepted. Does I1 on W368 name
  what can get slower, does I1 on a page change stay out of measuring, and what does T2 find on heads that measured
  only the read they meant to change, measured a slowdown the owner accepted, or changed nothing that runs?
- A defect's class and its prevention (#306 pinned git's hooks, #309 then the other programs git runs, W344 found four
  more). R1's `cause` asks, for a defect, for what keeps its kind from coming back and its like problems: the change
  covers those in this project's code, the rest go to side reports with why. T3 checks the change covers what the
  cause, or the design, says it covers. What do R1 on W339 and W344, and on a plain defect, do; and what does T3 find
  once R1 sent like cases to side reports?
- R0's purpose: it gathers history so that the work learns its lessons and does not take again a path that failed.
  Does R0's brief on W321, W324, W340 and two ordinary works (W350, W356) name the lessons, and what does it read where
  the project keeps a design tree?

Besides, R1's line on probes that showed nothing was replayed in a new wording on W349 (section 4.5).

## 2. What changed in the messages

- R0: its task becomes "This round gathers the history of this item, so that its work learns what was learned before
  and does not take again a path that failed: ... the approaches dropped, with why; the decisions overturned, with what
  replaced them, the git history of the project's decision files among them; ... Give the address of each with the
  lesson it holds, ...". The part on `decisions/` is as it was. Its brief is sectioned by lessons first: paths that
  failed or were dropped, decisions overturned.
- R1: the description of `cause` says, for a defect, its root and what keeps its kind from coming back, and of its
  like problems, the same mistake or the same kind of hole here or in other projects, that the change covers those in
  this project's code, and those it does not take, and those in other projects, go into side reports, saying why. The
  description of the change says it covers the like cases the cause says it covers, and what keeps the kind from coming
  back.
- I1: for a work that delivers a change, "Where the change can alter how much runs per operation or request, or how
  that grows with the data, measure with measure what it can make slower as well as what it is for, before and after
  the change, at the size the data has and at a larger one." No field: the engine keeps each run of `measure`.
- T2: "where the change can alter how much runs per operation or request, or how that grows with the data, was what it
  can make slower measured as well as what it is for: a measurement missing there, or a slowdown measured that the
  design or the owner did not accept, is a finding", and two inputs: every measurement the engine recorded for the
  work, and the measurement workflows the project lists ("none when it lists none, and then no measurement is
  missing"). The T2 replays below ran with a wording before the last: "where the project can measure (`measure`
  refuses where it lists no workflow)" in place of the input of the workflows, and no words on an accepted slowdown.
  They were not run again: every case replayed is of this project, which lists a measurement workflow, so the input
  says what the earlier wording assumed, and on W353 T2 already left the slowdown the owner accepted alone.
- T3: "for a defect, whether the change covers the like cases the cause, or the design where there is one, says it
  covers, and what keeps the kind from coming back".

## 3. How the rounds were run

As in [r1-defect-replay.md](r1-defect-replay.md): each round rebuilt from `rounds.input`, the recorded command line run
in a scratch clone at the round's commit with later refs removed, a new session, no hooks, scratch directories in place
of the trial's. Each case reads a copy of the trial database with every change after the original round undone from its
`changes` table, and rounds, tool runs and changes after it deleted; the message's database path points to the copy.
I1 ran in fresh sessions (the originals went on in R1's session, which lives in the engine's data directory); their
messages give R1's understanding, plan and acceptance from the record, as the engine gives them to a fresh I1 session.
T2's new input is given as the engine gives a database input, from the copy. The `ci` and `ci_before` runs the rounds
asked for ran through the fast CI on branches named `replay/...`; no `measure` ran. A few returns were refused by the
command line for their form (`outcome` written as a string, or `progress` missing), which the engine sends back to be
written again; their decisions are read from the last attempt. Cost: about 30 USD in all.

## 4. Cases

### 4.1 R0

| Work | Original and old prompt | New prompt |
| --- | --- | --- |
| W321 (#286, check the format before ci) | Original: "#109 allowed `cargo fmt`", missing that #266 (4110a676) refused it again; R1 found that in the code, and the work went to deep research. The old prompt today found #266. | Found #266 too, under "paths turned down: why one line in AGENTS.md is not enough", with the earlier refusal of `npm --version` and the owner's past ruling that permissions are the owner's to change |
| W324 (#290, do not close a merged PR) | #267's check before merging, observation 67, listed as related | The same, with the lesson written out: the PR's state is read when the action runs, not when it is queued; says no dropped approach is recorded |
| W340 (#307, record the engine's starts and stops) | Related items, the owner's #211 decision on retries | Also matter 811, where the owner said the 9 to 10 minute gaps were the engine stopped for an install, against the item's premise that their cause is unknown; the existing global offline stop (88e4ca20); the owner's #32 rule of stops by cause |
| W356 (#223, quote a chat bubble) | Draft layer, the owner's decision, the draft designs dropped in #222 | Same ground in fewer words, lessons first ("what of the kind is already merged"), and the owner's word in matter 437 that the item's premise of long chats is to be checked |
| W350 (#311, stale ci_before results) | Same returns sent back, related items, owner decisions | A section of paths dropped or replaced (matter 600's correction of how ci_before waits; #288 dropped for the 14900K fast CI) |
| W392 (#285, do not send back a late answer), at 24b01f22, the commit that brought `design/` | Cites the design tree's files as they stand (`design/wf-agents/flow/asking.md`, `returns.md`, `engine/test.md`) and the failed rounds R247-38, R242-38 | Cites the history of the decision files (f4160e05 added one, aa58fdb4 deleted #261's three), a third case of the same wrong return #285 does not cover (side report 333, three rounds), and the owner's ruling in matter 308; no file of the tree as it stands |

The briefs of W321 to W392 ran with a first wording of the task that also said "the design tree as it stands the work
reads itself"; W392 ran with the wording in the change. Judgment: the new briefs put the lessons first and say them,
where the old ones list addresses; W340's brings a fact the work needed and the old missed. At a commit with a design
tree, the new brief reads the history of the decision files and leaves the tree as it stands to the decisions in force,
which R1 is given anyway.

### 4.2 R1

W339 (#306, pin git's hooks in the engine):

| Run | Like cases | Prevention | Side reports |
| --- | --- | --- | --- |
| Original R339-5 | `core.sshCommand`, `core.fsmonitor`, merge and filter drivers, `credential.helper`: "not in this item's scope" | a constant of pinned settings, "one line to add one" | yes |
| Old prompt | `core.sshCommand`, `core.fsmonitor` | the setting in a constant | yes |
| New, a first wording | `core.sshCommand`, `core.fsmonitor`, "not handled in this item" | the setting in a constant | none |
| New, the wording before this review's | `core.sshCommand`, `core.fsmonitor`, `filter.*`, not taken: pinning `core.sshCommand` may break the owner's own SSH | one entry for the engine's git, used by every site, and a test that fails when production code builds a `git` command elsewhere | yes, saying why |

With the first wording R1 reproduced the hooks with three runs of `ci_before` (fast CI runs 42344499159, 42344657630,
42344807464) and named the like cases without taking or reporting them. The later wording names the kind and its
prevention and reports the other programs with why. The wording in the change only narrows it further (like cases in
other projects go to side reports); W339's like cases are all in this project's code, so it was not run again on W339.

W344 (#309, pin the other programs git runs), whose class this is:

| Run | Decision |
| --- | --- |
| Old prompt (main's) | reproduced eight settings with one test (fast CI run 42347886509), then started: pin the fixed names, a configured value for `core.sshCommand` and `credential.helper`, attributes' source pinned against named filters and drivers, every git call through one entry; side report: `remote.origin.url` and `insteadOf` redirect the engine's push, which no pin stops |
| New prompt | reproduced 12 settings one test each with a probe of each pin (fast CI runs 42347888401, 42348103235, 42348286519), then started: the same coverage, `attr.tree` and `core.attributesFile` pinned so that no driver name need be listed, the owner's own `core.sshCommand` and `credential.helper` read once outside the repository and pinned, a test scanning production code for any other `git` command, the pin probes taken out, their results kept in its return; side report: what no `-c` can pin (`credential.<url>.helper`, `insteadOf`, `remote.origin.url`), for #284's boundary |

W316 (#281, delete a merge's leftover file), a plain defect:

| Run | Decision |
| --- | --- |
| Original R316-3 | delete the file, `*.orig` in `.gitignore` with a comment |
| Old prompt | the same; "no other such file is tracked, so no like case to list" |
| New prompt | the same two lines; names `*.rej`, `*.bak`, `*~`, `.DS_Store` as not tracked on main, "not handled here, no item opened" |

Judgment: R1 names a defect's kind and what keeps it from coming back, covers the like cases in this project's code
(W344) or reports them with why (W339), and does not grow a small fix (W316). On W344 the old prompt already covered
the class; the new adds the guarding test and takes its probes out.

### 4.3 T3 on #306's head, after a T2 pass

T2 then T3 ran on W339's merged head (49fe8b11) with the new prompts, given the new R1's understanding and plan from
section 4.2 (which sent `core.sshCommand`, `core.fsmonitor` and `filter.*` to side reports) in place of the original's.
T2 found one thing: the comment says every git command of the engine goes through one entry, while seven production
sites still build their own (the head did less than the new plan). T3 raised nothing on the like cases the cause had
sent to side reports, and held the merge for T2's finding.

### 4.4 I1 and T2: what was measured

| Run | Decision |
| --- | --- |
| I1 on W368, original R368-12 to 20 | plan tests only, through `ci`; no `measure`; handed in done |
| I1 on W368, first wording ("... and hand in the numbers in measured") | after two rounds of `ci` and `ci_before` (runs 42344705099, 42345174003, 42345191612), asked `measure` of the read it sped up, at 1 and 10 times the data |
| I1 on W368, wording in the change | its first round wrote a measurement of the next transaction number and of an insert into the change log (what the new index can make slower), at the field's size and ten times it, and plan tests for other reads that use the new indexes (the merge queue's, the counters'); it did not name `work_routes`, where #316's slowdown was |
| I1 on W356 (#223, quote a chat bubble), wording in the change | built the page change and its tests; no word of measuring |
| T2 on W368's merged head, old prompt | one finding: a read still scans `sessions` |
| T2 on the same head, given only the target read measured | that finding, and "no measurement covers the write path the new indexes add to; measure timed only the read" |
| T2 on W353 (#240, the works page), which measured the works list read and asked the owner, who accepted a 3.8% slowdown | old and new: no finding; the new one names the slowdown as measured and accepted |
| T2 on W356 (#223), which changes no read or write | old and new: no finding |

The W368 T2 run with only the target read measured was given, as its record of measurements, the request the I1 replay
handed in (round 3 of its chain) with no result, since that 45-minute run was not made; it also noted the missing
result. Judgment: I1 now names what a change can make slower beside what it is for, and does not measure where nothing
runs differently; T2 raises a measurement of the target alone, and says nothing on a measured slowdown the owner
accepted or on a change that runs nothing more.

### 4.5 R1's probe line on W349

Within the round bound, "a probe there that showed nothing is taken out once run" held in one run of two (PR #98).
With "is taken out in the round that sees its result", two runs of two took the probes out and wrote the note (fast CI
run 42344422210 on `replay/w349-probe-clause`: all eight tests passed, nothing reproduced), and both asked the owner.
The change carries it as "... in the round that sees its result, its result noted in `research/`".

## 5. Conclusion

I1 now measures what a change can make slower beside what it is for, and leaves alone a change that runs nothing more;
T2, reading the engine's own record of measurements, raises a measurement of the target alone. R1 names a defect's kind
and what keeps it from coming back, covers the like cases in this project's code or reports them with why, and does
not grow a small fix; T3 holds the change to what the cause says it covers. R0 writes the lessons of the history first,
the history of the decision files among them.
