# The reviews and implementing replayed with what the reviews check told first

For: the owner, who approves the change to cells T2, T3, I1 and T6 (and the inputs of T1 and A1) at the merge, and its
reviewer.
Kept: until that change merges or is dropped; then deleted, git keeping it.
Written: 2026-11-07, on the branch `reviewers-and-conformance`, from the trial database as it stood that day.

## 1. Question

Two problems of the engineer's flow, as `engineer-problems` numbers them:

- Gap 8: the reviews judge without what they need. The details review (T3) opened 10 visits by querying `work_ci` for
  the CI on the head; the upper review (T2) looked up the engine checks the engineer said it changes (`checks_changed`)
  in 4 visits (W308, W309, W318, W329); flow feedback 306-26 and 316-8 said no CI result was among the inputs.
- Gap 14: the code is not checked against the design and the decisions after implementing, and implementing declares
  its departures (`deviations`: `design_changed` 55 times in 25 works, `workaround` 19 times in 17) for the review to
  challenge instead of following the design or writing the decision into it.

With the reviews given the CI on the head and the checks the engineer says it changes, T2 told to check the code against
the design and the decisions first, and implementing told first to solve the item's problem as the design says and to
write a decision the design does not record into it (`design_written`, named `design_updates` after the review), does
the reviews' lookup of the CI go, does T2 check the three points without raising what implementing already wrote into
the design, and does implementing amend the design or fix the code instead of declaring? And, after the review: does T2
judge what was written into the design as design, and does the fix write into it too?

## 2. What changed in the message

As `flow/flow.json` on the branch first worded it; every other word is the recorded round's. Sections 4 to 6 ran with
this first wording, in which the field was `design_written`, T2 was not told to judge the passages and the fix kept
its departures; section 9 gives what the review changed and the rounds run on that.

- T2 and T3: input "What implementing handed in" reads `design_written` and `checks_changed` of I1 in place of
  `deviations`; a new input after the review base, "The CI on the head under review", from the engine's `head_ci`: each
  run of ci on the head the work last pushed, newest first, as the check that the head is green reads them.
- T2's task: its check line becomes two. First: does the code solve the item's problem as the plan or the design says,
  with the passages implementing wrote into it, and to the standard the decisions in force set (the work done once that
  problem is solved); does the code hold a decision neither the design nor the decisions in force record; does the code
  the work changed break a decision in force anywhere, old code elsewhere lagging behind being no finding. Then the old
  checks, unchanged.
- I1's task: the line "A small case the design did not foresee you handle yourself, and write it as a departure. ..."
  becomes: solve the item's problem as the plan or the design says; a decision the code makes that neither it nor the
  decisions in force record, or a detail it got wrong, goes into the design in `design_written`, with why; a design
  wrong in its direction goes back to research; a workaround for a problem outside the work is a side report with a
  comment in the code. The sentence "With your answer, write where you departed from the plan, as surprise, workaround
  or design_changed" goes. I1's format: `deviations` goes, `design_written` (passage, decision, why) comes.

## 3. How the rounds were run

As in `research/r1-defect-replay.md` (branch `research-reproduces-defects`, merged as #98): each round rebuilt from
`rounds.input`, the recorded command line run by hand in a scratch clone at the round's head with `origin/main` at the
main it saw and every later ref removed, a new session, the default Claude configuration directory, the `--settings`
hooks left out and the trial's `--add-dir` directories pointed at empty scratch directories. Old and new ran the same day
under the same conditions; the original return is given beside them. Further:

- T2 ran in a new session, as the review line starts one there. T3 ran in the session the T2 replay of the same variant
  left (`--resume`), as the line continues it, with the recorded T3 message; so the new T3 was referred to the CI on the
  head by its address, as the engine refers a session to what it holds.
- What the new variant gives in place of implementing's departures was made from the recorded ones: each
  `design_changed` row as a passage of `design_written`, workarounds and surprises dropped. The CI on the head was
  computed from `tool_runs` as `head_ci` computes it, cut at the round's start.
- I1 ran at its hand-in round as a fresh session gets it: every input given whole, taken from the rounds that gave it,
  and the latest run of ci on the head as the tool results it came with. The original sessions are in the live data
  directory's Claude configuration, which this work did not touch, so a resumed round could not be rebuilt. A fresh
  session reviews the branch it takes over, and both variants changed code; that is the setting's effect, not the
  change's.
- Queries were run on a copy of the trial database cut at the round's start. `ci` and `ci_before` were not run (they push
  to the project's repository): a request for them was answered as passing, each test as the work's recorded runs
  reported it or, for one they never reported, passed. W344's old variant noticed that a `ci_before` answered so passed
  where it should have failed and said so; no return below rests on such a result otherwise.

Cost of all replays, both rounds: about 28 USD.

## 4. The reviews

### W316, #281 (drop a stale `.orig` file and ignore such files), haiku, straight from the research

| Run | T2 | T3's first return |
| --- | --- | --- |
| Original R316-7, -8 | no findings | queries `work_ci`: "the inputs do not say whether the CI run on the head passed" |
| Old | no findings | queries `work_ci`, with the same feedback |
| New | no findings | hands in at once: nothing to fix, the head's lint and fast CI green, as the input gives them |

### W319, #15 (the hook gives no interjection the round's message gave), sonnet

| Run | T2 | T3's first return |
| --- | --- | --- |
| Original R319-13, -14 | one finding (U1, `delivered_round` stays empty through the round) | queries `work_ci` and `gh pr view` |
| Old | no findings | hands in, no lookup |
| New | no findings | hands in, no lookup |

Neither replay raised the original's U1; that is the model's variance on the same message, not the change.

### W321, #286 (run `rustfmt` before `ci`), haiku, designed; implementing changed one sentence of the design

The design's sentence gave a reason for not using a glob that the research had not tried; implementing left the reason
out and declared it as `design_changed`. The new variant gives it as a passage of `design_written`.

| Run | T2 | T3's first return |
| --- | --- | --- |
| Original R321-35, -36 | U1 (`rustfmt` does not read `rust-toolchain.toml`), U2 (the query's threshold `key > 30`) | `gh run list` (the CI) and `gh pr view` |
| Old | the `rustfmt` finding only | four queries and `gh run list` (the CI) |
| New | U1 (the report's queries have no time bound), U2 (the threshold, and "never refused" written as measured), U3 (`rustfmt`) | `gh pr view` and a query of the item's asks; no CI lookup |

Neither variant raised the passage implementing wrote into the design. Neither T3 replay said the first pass's findings
came by number only (the original's feedback in R321-36 and -38): the session holds them as its own return.

### W344, #309 (pin the programs git's configuration can name), sonnet; implementing added `GIT_ASKPASS` beyond the plan

| Run | T2 | T3 |
| --- | --- | --- |
| Original R344-13, -14 | B1 more entry points left open (`ext::`, signature checks, `uploadpack.packObjectsHook`); B2 the costs written nowhere but the code; B3 `probe.sh`'s header claims a run it does not do | one more finding (A1's fetch untested); adversarial |
| Old | signature checks; `uploadpack.packObjectsHook`; `probe.sh`'s header | no new finding; adversarial |
| New | `diff.external` not pinned on `git diff`; signature checks; `ext::` | `probe.sh`'s header; adversarial |

The new T2's findings are all of the first check: the item's problem (the engine's git runs no program the
configuration names) not solved wherever it stands. The `GIT_ASKPASS` passage implementing wrote into the design was
not raised. No T3 replay of W344 looked anything up, nor did the original.

### Judgment

The CI lookup goes where the old variant made it: W316 (`work_ci`, as the original) and W321 (`gh run list`, as the
original). T2 checks the code against the design first and raises what the design and the item ask that the code does
not do (W344, W321), and in no case raises the passages implementing wrote into the design. The remaining first
lookups of T3 are of the pull request's description, which this change does not give.

## 5. Implementing

### W344 (sonnet). Original R344-12: `design_changed` (`GIT_ASKPASS` and `GIT_TERMINAL_PROMPT=0` added, with a test), two surprises

| Run | What it did before handing in | What it handed in about the design |
| --- | --- | --- |
| Old | four more pins with tests (signature checks, signed push, `ext::`), `probe.sh` deleted, acceptance A10 added | `deviations`: two `design_changed`, a `workaround` (the new pins not tried locally first), two surprises |
| New | `probe.sh`'s header corrected (it claimed what only a test showed), a paragraph added to the decision on commands the engine runs | `design_written`: the `GIT_ASKPASS` passage, with why, the original's `design_changed` word for word in substance; no surprise |

### W354, #232 (take back the owner's words no agent has taken up), sonnet. Original R354-21: two `design_changed`, two surprises

| Run | What it did before handing in | What it handed in about the design |
| --- | --- | --- |
| Old | extended a test, then handed in a return of no form three times (no return) | none |
| New | extended two tests (a word that cannot be taken back keeps its head; a withdrawn matter is not taken back) | `design_written`, four passages: relayed answers are not taken back (the original's first `design_changed`), words to both lines, what counts as the architect's pick, and Take back shown while the queue's pick runs, refused in the write |

### W321 (haiku). Original R321-34: `design_changed` (the design's sentence without its unverified reason), two surprises

| Run | What it did |
| --- | --- |
| Old | read the records and the PR, then began building the engine approach the owner had withdrawn, which the locked acceptance still described (four engine files changed, a module added); stopped by hand before it handed in |
| New | read the owner's interjections, then asked the architect to lock the acceptance to the design's four conditions, since the locked list was the withdrawn engine approach's; the original had noted the same as a surprise and handed in |

The fresh session met here what the original met: the design (an `AGENTS.md` line) and the locked acceptance (an
engine check) disagree. The new variant stopped at it and asked; the old one followed the acceptance against the
design. One case, so a hint, not a finding.

### Judgment

Where the old prompt declared, the new one wrote the decision into the design with why (W344's passage is the
original's `design_changed`; W354's four include the original's first) and fixed what was wrong instead of noting it
(W344's `probe.sh` header, which the original's review later raised as B3). Surprises were not handed in: what the
originals said as surprises went into the design (W354), the PR description, or nowhere. The paragraph W344 added to a
decision of `decisions/` is one its acceptance (A8) asks for, and the old variant added it too.

## 6. The trial's departures, read again

All 74 `design_changed` and `workaround` rows of I1 in the current graph's works (W301 to W374) were read; what each kind
would now be:

- A decision the design did not record or a detail it got wrong, with a reason (about 50 of the 55 `design_changed`:
  W306's five on how findings set aside are judged, W308's waits and limits after the merge, W312's and W313's choices
  of where a rule is defined, W329's kind of matter, W344's `GIT_ASKPASS`, W354's kinds of words not taken back): a
  passage of `design_written`, the same words, which the review now reads as part of the design rather than as a
  departure to challenge.
- A change of direction (W318 built the research's plan instead of the deep research's design): back to research, as the
  line on a design wrong in its direction says.
- A choice too small to be a decision of the design (W308's function name, W313's helper's prefix, W340's test placed in
  another file): nothing, or a comment in the code.
- A workaround for a limit of the engineer's environment (W328 and W330 could not compile locally, W331 and W365 could
  not run `jq` or read some tables, W338 could not run `sqlite3`, W353 could not run `awk`, `node` or `python3`): a side
  report, which reaches the architect, and a note where the work's result depends on it.
- A workaround inside the work to make a test or a check work (W345's literal key name so that `ci_before` compiles, W368's
  red evidence taken from a revert commit, W324's wait condition, W340's split of a function for its fault path): a
  passage of `design_written` with why, or a comment in the code where the choice is a line of code.
- Surprises (83): nothing, unless the owner should know, in which case `tell_owner`, the report or the PR description.

## 7. The re-check (T4)

The owner's settlement asked for the first pass's findings in full at the re-check. T4 is given them already: its input
"What the fix answered" is the table the fix answered (`fix_table`), each row with its number, where it came from
(the findings of T2, of T3, ...) and the finding whole (number, where, what, impact, basis, fix), beside the fix's
answers; R321-42 got U1, U2, D1 and D2 so. No T4 round's feedback in the current graph's works (47 visits) said a
finding was missing. The feedback cited (R321-36 and -38) came from T3, the second pass, which continues T2's session:
its input "The first pass's findings" was referred to by address because the session holds them as its own return, and
the round said it took them from there. No T4 change is made, and none was replayed.

## 8. Conclusion

The reviews' lookup of the CI on the head goes where it was made, at the cost of the input itself (the runs on the head
were 3.0 thousand characters at the median of the trial's 41 upper reviews, 9.5 thousand at the 90th percentile; the
details review is referred to it). T2's new first check finds what the item asks and the code does not do, and leaves
what implementing wrote into the design alone. Implementing writes the decisions it makes into the design and fixes what
is wrong instead of declaring departures. The replays of implementing ran as fresh sessions at the hand-in, which made
both variants review and extend the branch; the comparison holds between the variants, not against the original rounds.

## 9. After the review: the passages judged, and the fix writing into the design too

What changed, as `flow/flow.json` now words it:

- The field is `design_updates`, the design as updated while implementing or fixing, a shape the fix (T6) hands in too in
  place of its departures, which go from the flow. Each hand-in carries every passage its cell has written in the work,
  earlier rounds' included: an input reads a cell's newest return only, so a passage left out of a later return would be
  lost to the cells after it. The field's description ends "The reviews judge these passages with the design".
- T2's first check judges each passage implementing changed or added as it would the design: a detail the item's
  problem needs, within the design's direction, with a reason that holds; a short cut, another method or more than the
  work needs is a finding, and a change of direction should have gone back to research.
- T6 gets the line I1 has (follow the design; a decision it does not record goes into `design_updates` with why; a
  design wrong in its direction is asked about, as the fix has no way back to research of its own; a workaround outside
  the work is a side report), and its hand-in line no longer says "as when implementing ... where this round departed".
- T1 (adversarial testing), T6 and A1 (the advisor) read both cells' passages with the design; A1's input of departures
  goes.
- I1's first line ("Change the code by the plan or the design ...") and its line on following the design are one.

### W318, #270 (an old-behaviour test that cannot be built before the change), sonnet: a passage that changes direction

No past case had a passage that is only a short cut, but W318's is one and more. Its implementation built the research's
plan instead of the deep research's design, leaving out the design's changes to T3 and T4, and declared it as a
`design_changed` with a reason (the owner had said to start; the adversarial tester's rows have no reason field). The new
variant gives that row as a passage of `design_updates`, with I1's two `checks_changed`; the old variant is the recorded
message. Both ran in a new session.

| Run | T2 |
| --- | --- |
| Original R318-32, -34 | queried `checks_changed` and `work_ci` first; then U1 (the design's T3 and T4 changes not made, though four texts say the review judges the reason), U2 (the adversarial tester's `cannot_run` let through, which the design kept sending back) |
| Old | the same two (U1, U2) and a stale doc comment (U3); no lookup |
| New | U1 the rule on a missing reason holds nowhere on the adversarial way (what the design's table asked and the code does not do); U2 judges the passage itself: its reason does not hold (the old rule had the tester fix its test, not write a reason), letting the tester's `cannot_run` through is more than the problem needs, and the design kept it sent back; U3 the same stale comment; no lookup |

The new T2 judged the passage as design and raised it, citing the reason the passage gave and the rule that more than
the work needs is a finding. It did not say in so many words that the work should have gone back to research; it said the
design kept the rule and asked that the code follow it.

### W319, #15, the fix (T6) at its hand-in, sonnet: a departure the fix declared

The original R319-24 answered five rows `fixed` and declared one `design_changed`: the hook now records what the round's
message gave as delivered, which the research had left unrecorded until the round ends, after adversarial testing found
the owner's page showing words the engineer had read as not taken up. As for I1, the round ran as a fresh session with
every input given and the recorded run of ci on the head.

| Run | What it did |
| --- | --- |
| Old | changed the code again and asked for ci on a new head (stopped there: ci is not run in these replays) |
| New | handed in at once: the same five rows `fixed`, the head as recorded, and one passage of `design_updates` saying the original's `design_changed` (the hook records what the message gave as delivered, before the round ends, with why) |

### Judgment

T2 now judges a passage as design: W318's, a change of direction written as a passage, was raised for its reason and for
being more than the problem needed. The fix writes its decision into the design as implementing does (W319). The
item page draws the passages inside the design block; that part was not replayed, being the page's, and is held by the
page's tests.
