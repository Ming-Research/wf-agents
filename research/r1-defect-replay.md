# R1 replayed with the defect rule and the research folder rule

For: the owner, who approves the change to cell R1 (Initial research) at the merge, and its reviewer.
Kept: until that change merges or is dropped; then deleted, git keeping it.
Written: 2026-11-06, on the branch `research-reproduces-defects`, from the trial database as it stood that day.

## 1. Question

Does the changed R1 prompt (a) stop a work that fixes a defect it has not seen, or make it see the defect where it
shows, and (b) leave works that fix no defect as they were? Two cases test (a): W332 (#300), which started on an
inferred cause and whose defect came back as #302, and W349 (#302), whose research could not run a Chromium probe
because R1 could write only `research/`. Three cases from 2026-11-05 test (b): W356 (#223, a page feature), W345 (#310,
an engine feature) and W350 (#311, an engine and flow change).

## 2. What changed in the message

Only the task of R1, as the change words it in `flow/flow.json`. The first line

> This round researches: change no file outside `research/`, where code that reproduces the problem goes, committed
> locally.

becomes

> This round researches. What you make for the problem, data, code, probes and notes, goes in `research/` and nowhere
> else, committed locally; a test that reproduces the problem goes where the project's tests are, so that it runs
> through ci and the implementation keeps it; a simple research leaves nothing but its conclusion.

and after the line that says to reproduce the problem (unchanged) comes

> A defect, though, is fixed only once seen to happen where it shows, as on the page in a real browser or in the
> program on its own data: by a test run there with ci or, where no test can show it, by a probe, a log or the owner's
> own run; a cause read from the code, or a test in a stand-in such as a simulated page, has not seen it. If, after
> looking as far as the tools reach, it is still seen nowhere, neither start nor hand it to deep research: ask the
> owner how to go on.

A first wording of that sentence (`When the work fixes a defect, reasoning is not enough: see it happen where it shows
..., by a test or, where none can show it, a probe, a log or the owner's own run; ...`) was replayed on W332 first and
did not hold: the agent read the code, wrote "the root cause is confirmed by reading the code", planned a happy-dom
test as "a test", and started (the row "New, first wording" in section 4). The wording above names the stand-in and
the cause read from the code as not seeing it, and names ci; it is what the change carries.

## 3. How the rounds were run

Each case is the first R1 round of the work, rebuilt from `rounds.input` (the message word for word and the whole
command line), with the stored contents put back. The command line ran as recorded, with these differences, none of
which reaches the agent's instructions:

- the working directory is a scratch clone of this repository at the commit the round ran on (head and main were the
  same commit in every case), on the work's branch, with no remote, `origin/main` at that commit and every later ref
  and reflog removed; the message's worktree path is put to it;
- a new session id; the default Claude configuration directory instead of the data directory's;
- the `--settings` hooks left out: they ran the trial's binary against the trial's data to deliver interjections, and
  re-gave the inputs on compaction, which no replayed round needed;
- the `--add-dir` directories of the trial pointed at empty scratch directories.

Both prompts ran the same day under the same conditions; the original return is given beside them. Where a round asked
for `query`, the queries were run by hand, read-only on the trial database, limited to rows from before the original
round started, and given back in a resumed session as the engine gives tool results, only where the decision was not
yet visible. No `ci` was run: it would push to the project's repository. Cost of all replays: about 5 USD.

## 4. Cases

### W332, #300 (fix Space on the flow chart), commit aab695d5

At this commit the project's CI had no real browser; the page's tests ran only in happy-dom.

| Run | Decision | What it rests on |
| --- | --- | --- |
| Original R332-3 | start | cause "read from the code, not reproduced in a browser"; a happy-dom test to go red then green |
| Old prompt | start | cause "confirmed by the code, the phenomenon not reproduced in a browser"; `:hover` fix, happy-dom test |
| New, first wording | start | "root cause confirmed by reading the code"; tells the owner it was not seen in a real browser and starts anyway |
| New prompt | ask the owner | read the code, then looked for Playwright, Puppeteer or Chrome in the workflows and the lock file, found none |

The new prompt's ask: the defect was not seen in a real browser; what was found (the `hovered` flag set only by
`pointerenter`, the remount after layout, commit 0a304e02); why it cannot be seen (CI has only happy-dom, which a test
there cannot settle); three options, fix on the inferred cause, first get the owner's browser and steps (with a log to
run), or drop it, recommending the first with its risk stated. The work holds at the owner's cell, at the research
stage, until the owner answers.

Judgment: the rule does what #302 needed. Under it the owner is told before any code that the fix rests on an inference
and decides; on the old prompt the work started both times, and on the first wording too.

### W349, #302 (find the real cause of the Space failure), commit d6ec1b51

At this commit the CI runs `client/src/**/*.browser.test.tsx` in Chromium (#303).

| Run | First return | Decision |
| --- | --- | --- |
| Original R349-5, -7, -9 | queries, queries, then start | started with no reproduction: "the research round may change only research/, and a browser test must be in client/src/, so the probe goes to the implementation" |
| Old prompt | queries (engine runs, matters, interjections) | not reached in one round; the same first step as the original |
| New prompt | `ci` naming `client/src/graph/Viewer.browser.test.tsx` | wrote and committed five Chromium scenarios (a click on empty chart space, on a cell, on a page control, the pointer out and back, a cell clicked then the pointer out and back) and runs them before deciding |

Judgment: the research now reproduces where the defect shows, in its first round, instead of handing an unreproduced
cause to the implementation. The probe went into the existing test file, where the implementation keeps it.

### W356, #223 (quote a chat bubble), commit 017903a4

| Run | Decision |
| --- | --- |
| Original R356-3 | start: the draft layer's `putDraft` and the bubble are there; add an entry and a quote block |
| Old prompt | start, the same plan; a markdown unit test and a happy-dom test |
| New prompt | start, the same plan; says plainly "a new capability, not a defect"; adds a Chromium test only for keeping the text selection when the button is pressed |

No file was left in either checkout. Judgment: unchanged; the defect sentence did not apply and was not applied.

### W345, #310 (record the model's time of each round), commit 95da86db

| Run | Decision |
| --- | --- |
| Original R345-7 | start: add the field where the `result` line is read |
| Old prompt | a query of `rounds.usage` first |
| New prompt | a query of `rounds.usage` first (whether `duration_api_ms` is there for I1 rounds); given the answer (547 rounds, none with it), see below |

Given the answer, the new prompt's second return starts: "a new capability missing one field", the read point in
`read_stream`, the field written only when the `result` line has it, a test red before and green after; the same plan
as the original.

Judgment: unchanged. The first query is no effect of the change: the old prompt asked one too, and the original did not.

### W350, #311 (tell the engineer a ci_before result is stale before it hands in), commit d6ec1b51

| Run | Decision |
| --- | --- |
| Original R350-3, -5 | queries counting the returns sent back, then start |
| Old prompt | queries counting the returns sent back |
| New prompt | a query counting the returns sent back; given the answer (11 rows up to the original round), see below |

Given the answer, the new prompt's second return starts: `ci` tells, before the table is handed in, which results of
`ci_before` are stale, the check itself unchanged, and the other class (`ci` not reporting a named test) left alone
since none came after 139948a9; the same way as the original R350-5. Its problem cites R328-22 from the records, where
the rejection was seen to happen.

Judgment: unchanged; no round asked the owner.

## 5. Conclusion

Both defect cases change as intended: W332 stops and asks the owner rather than start on a cause read from the code,
and W349 reproduces in a real browser through ci, which the old limit to `research/` forbade. The works that fix no
defect take the same way as before. The first wording of the defect sentence failed on W332, so the wording replayed
second is the one in the flow. D1, D2 and I1 were not replayed: D1 and D2 take the same sentence on the research
folder that R1 took here, and no past work had a reproducing test left by its research for I1 to take.
