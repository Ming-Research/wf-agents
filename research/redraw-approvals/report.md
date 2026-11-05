# Owner approvals that direct pushes to main removed or changed (item #299)

Delivery of item #299. The owner's words are in Chinese in the database and the repository holds English only, so every
quotation below is a translation by the author of this report. The original of each is read with
`SELECT body, answer FROM owner_asks WHERE id = <ask>` on the engine's database.

## Answer in short

Between 2026-11-03 19:30 UTC and 2026-11-04 19:00 UTC, 96 of the 97 commits that reached main came by direct push, not
through a pull request. Of the 25 asks of the owner's that were answered in asks 25 to 49, the direct pushes took away or
changed the approval of 10 (asks 25, 26, 28, 30, 31, 35, 39, 40, 41, 47), which make the 8 rows A1 to A8 below. They left 7
untouched (asks 27, 29, 34, 36, 42, 48, 49). The other 8 never reached main or are not about the flow (asks 32, 33, 37,
38, 43, 44, 45, 46). Asks 2 to 24 and 50 to 79 were read too; none of them is such an approval (sections 6 and 7). The 8
rows, in one line each:

| # | Ask | Item | What was approved | Main at the window's end |
| - | --- | ---- | ----------------- | ------------------------ |
| A1 | 40 | #234 | The light route for bug fixes and small features, with four points that differ from the owner's earlier words | Gone: no light route; no cells G4, F1, F2, T7 |
| A2 | 47 | #247 | Cell D12 that waits for the ruling on a departure from a decision, and a per-decision table in the design | Gone: no D12; a departure no longer stops the work, the merge waits |
| A3 | 28 | #205 | Cell G3 (history of the changed files before the table review) and a finding for a change that walks a dropped path | Gone: G3 and the finding rule; the path-mapping rule stays |
| A4 | 31 | #228 | The architect answers similar reports together, in one round, up to 8 at a time | Gone: no grouping, no cell that picks matters |
| A5 | 26, 30 (and 25) | #195 | Decisions kept in the items under `Decisions:`, recorded from marked quotations, with ten wordings in the flow | Replaced: no decision line is written in an item; decisions go to `decisions/` |
| A6 | 35 | #219 | The architect removes a dependency; the change passes the batch review like adding one | Changed: the removal stays, the batch review is gone; only five kinds of change get a second look |
| A7 | 41 | #16 | Reviews told what changed since last time; CI result to the details review; the cost-under-load question as its own column | Changed: the load question and the table review are gone; the "since last time" input is partly kept |
| A8 | 39 | #217 | Five wordings on return forms; the third says when a locked acceptance changes and who changes it | Changed in the third: the lock and its check are gone; the engineer hands in the new list |

Who answered is in the table of section 4. A clear fact the records do not give: whether the owner knew each of these
would go when the redraw was pushed. The commits have no pull request, so no flow-page comparison of the kind several of
the approvals asked for before the merge (asks 39, 40, 42, 47) was made for them.

## 1. Window and how it was read

- Window: 2026-11-03 19:30 UTC (11:30 PST) to 2026-11-04 19:00 UTC (11:00 PST), as the item says.
- Direct push means a commit that no pull request brought to main. Read from the list of pull requests
  (`gh api repos/Ming-Research/wf-agents/pulls?state=all&per_page=100`, 2026-11-04) and from main's first-parent history:
  - Pull requests merged in the window: PR 56 (#250), merge commit `b73c0cbc`, 2026-11-04 17:17 UTC. Its flow change was
    asked of the owner as ask 57 and approved by the delegate.
  - PR 49 (#11) is marked merged at 2026-11-04 01:06 UTC, but its branch reached main by a push of the delegate's
    `integrate/six-branches` merge (`49f05b9a`, 2026-11-03 16:49 PST), and GitHub marked it merged afterwards. It counts as
    direct.
  - No other pull request merged in the window; PR 57 (#208) merged at 20:10 UTC, after it.
  - Counts: 97 commits in the window (`git log origin/main --since --until`), 47 of them on main's first-parent line; 46
    commits touch `flow/flow.json`, 45 of them direct.
  - A trailing `(#N)` on a commit of the window such as `(#266)` or `(#273)` is an item number, not a pull request.
- All commits are authored by the one git user, so the commits do not tell whether the owner or the delegate pushed.
- Three revisions of the flow were compared (git worktrees of each, read with `jq`):
  - START `a5143988`, the last main commit before the window (PR 48, 2026-11-03 09:59 PST). Every approval below that
    reached main before the window is in it.
  - END `0ecb0d81`, the last commit in the window (2026-11-04 10:52 PST).
  - NOW `9228c1f7`, main when this report was written, to see what pull requests after the window did.
- The redraw itself is the series `b055a7f4` to `ccededea` (2026-11-04 05:09 to 08:35 PST) plus the commits around it, most
  marked "work in progress". The flow changes in the window before it (2026-11-03 11:42 to 16:57 PST: #215, #247
  compatibility, #268) were also direct; no approval of asks 25 to 49 is among what they changed.

## 2. What the direct pushes did to the flow

Cells by id, START to END (`jq '.flows[].nodes[] | [.id, .title]'`). An id can mean a different cell at END; the titles
decide.

Engineer graph, 46 cells at START and 32 at END (`jq '[.flows[].nodes | length]'`: 46 and 32 for the two graphs at START,
32 and 28 at END and at NOW):

- Gone with no cell of the same title: R2 "Whether it stands", G2 "Gather the code's history", D4 to D11 (the engine and gate
  steps for what is under test, sureness, cost, other projects, size and adversarial testing), D12 "Wait for the ruling on a
  departure", G4 "Gather like fixes", F1 "Plan the fix", F2 "Review the plan", G3 "Gather the history of the changed files",
  T7 "Choose what runs next", L1 "Tool: watch main and the deployment".
- Same id, other cell: G1 became R0 "Gather the background"; D1 "Compare approaches" became "Deep research"; D3 "Whether
  there is a better design" became "The review of the design"; T2 "Tool: the full CI on the draft" became "Review: the upper
  level"; T3 "Blind review" became "Review: the details"; T4 "Table review" became "Re-check".
- New at END: R0, D4 "Take in the review", D5 "Split along the design's parts", A3 "To the owner". (W1 and W2 are from a
  pull request after the window, PR 60; D4 and D5 are gone again at NOW, by a direct push after the window.)

Architect graph, 32 cells at START and 28 at END: gone are Q1 "Pick the next matter", H10 "What runs next", H12 "What was
said in the groups", B1 to B5 (the batch review and the recording of the owner's decisions), S1; new are L1/L2 (check
main), G1/G2 (the second eye), C2.

Engine checks of returns (`engine.checks`), 53 at START and 36 at END. 21 gone: `acceptance_matches_locked`,
`blind_findings_carried`, `changes_valid`, `claims_answered`, `claims_fit_verdict`, `columns_answered`,
`conclusions_cover_questions`, `decision_words_hold`, `decisions_answered`, `design_card_disagreement_redirects`,
`disputed_tests_answered`, `evidence_holds`, `group_answered`, `no_adversarial_when_due`, `no_insisting_after_ruling`,
`observation_changes_valid`, `picked_pickable`, `review_covers_changes`, `route_given`, `unverified_decided`,
`words_all_looked`. 4 new: `closing_given`, `decisions_written`, `must_fix_fits_findings`, `points_chosen`. The checks are
named in the rows below where an approval depended on one.

`db/schema.sql` changed by 217 lines; no changed line names the views `merge_queue`, `work_counters` or `work_asking`
(`git diff a5143988 0ecb0d81 -U0 -- db/schema.sql`, searched for those names: no hit).

## 3. Method for each approval

For each answer of the owner's in asks 25 to 49 that approves something, the approved content was read from the ask, then
looked for in the flow at START (to see that it had reached main) and at END and NOW (to see what stayed), by the cell and
field it names, or by a distinctive phrase (`probe.jq` counts the string values of a flow file that contain a phrase).
"Same" means the JSON value is equal at START and END (`jq -s '.[0] == .[1]'`).

## 4. Approvals removed, replaced or changed

Each row: the ask, the time it was answered (UTC, from the answer's time stamp), who answered, what was approved, main
now, whether the owner decided otherwise later.

Who answered, as the records say. Asks 25 to 44 carry no `speaker`; the field began with ask 45. For them the only marks are
the engine's `by: owner` and the words of the answer. Where the answer says "after the owner read the delegate's plain
explanation" or "the owner decided by the delegate's advice", the owner's page was not the source; whether the owner typed
the answer or the delegate wrote it as the owner's cannot be told from the database. Asks 47 to 49 are marked
`speaker: delegate`, answering "by the owner's authorization tonight" ("you approve everything tonight, do not hold
anything up"); the answers say so in their own words.

### A1. Ask 40 (2026-11-03 06:01), item #234 Add a light route

- Who: `by: owner`, "the owner decided by the delegate's advice".
- Approved, in translation: "Agree with all, implement as designed, send me the flow page's comparison before the merge",
  and, on the fourth point, "'the route is marked by the architect when it creates the item, and can only be changed back
  to normal, never the other way' is also agreed: the side that does the work cannot reduce its own checks". The four points
  differ from the owner's words of 11-03: (1) the light route starts with G1; (2) it has no blind review, only T4, by the
  reviewer who reviewed the plan; (3) a third return of the plan by F2 turns the work to the normal route; (4) a regression
  test red before and green after replaces adversarial testing, and R1, F1, F2 can only change the route back to normal.
  The route: G1, R1, G4, F1, F2 (back to F1 if not passed), I1, T7, T2, T4, merge.
- Merged as PR 40 (2026-11-03 13:20 UTC) and at START.
- Main now: none of the light route stays. G4, F1, F2, T7 are gone; the `route_given` check is gone. The engineer graph
  has one path from R1: the row `start` goes to I1, and deep research (D1) is taken only when R1 says so; this work went
  that way. What stays is vestige: the item's `route` mark (`normal`, `bugfix`, `feature`) in `db/schema.sql` and the item
  change, and the shape `route_taken` with text for "the light route". Nothing in the graph reads it as a route, so the
  rule "only the architect marks it and only toward normal" no longer governs anything: R1 itself decides whether the work
  goes the short way.
- Later owner words: the question of ask 47 (11-03 10:03, before the window) says the owner decided in matter 655 to
  replace the light route by two separate light flows, and that F2 is not wanted (not read in the matter itself); ask 65 (2026-11-04 22:46, after the window,
  delegate relaying "the others as recommended") records that the owner decided "tonight" that only R1 judges whether a work
  goes deep and that the short path is the bug-fix path. #258 and #259 were cancelled for it. Neither text is the owner's
  own typing in a record the database holds.
- Verdict: removed. Later words of the owner, relayed, go the same way.

### A2. Ask 47 (2026-11-03 10:03), item #247 Check design fit

- Who: `speaker: delegate`, "by the owner's authorization tonight".
- Approved, in translation: "(a) approve: the normal-route part as designed; before the merge give the owner the last
  comparison once more. The light-flow part goes to Add bug fix flow, agreed." What the ask describes: a new cell D12 where,
  when a design departs from a decision the item holds, the work waits after the design for the ruling of whoever made that
  decision ("yours is ruled by you, mine by me"), recorded as a new decision line; a table in the design (D2) that says per
  decision followed, departed or not involved; the design review (D3) hunting the passage that most likely overturns each;
  a redesigned design reviewed again; three wordings and one check on the architect's cells H3, B5, L2, so that a departure
  from the owner's decision can be answered only by the owner.
- Merged as PR 43 (2026-11-03 14:26 UTC) and at START (cell D12 present).
- Main now: D12 is gone from the flow (`jq` finds no cell D12; also the one-word `grep` the brief gave). The departure
  remains as a field: R1 and I1 return `departs` with `verdict yes`, the decision, why and what happens otherwise, and say
  "whoever made the decision rules on it, the work goes on meanwhile by what you recommend, and the merge waits for the
  ruling". So the stop at D12 is replaced by "the work goes on, the merge waits"; the per-decision table is gone with the
  checks `decisions_answered` and `claims_answered`; the architect's H3, B5 and L2 wordings are gone with H3's check.
- Later owner words: ask 77 (2026-11-05 00:25, delegate relaying) quotes the owner: "after D is done, do not stop, hang
  there and wait for the approval; if I approve something different, go back and do D again", and says it replaces the
  11-03 sentence "the engineer researches carefully but must stop and confirm before implementing". That covers waiting
  for the design review, not the ruling on a departure from a decision; for the departure the owner has said nothing later.
- Verdict: replaced by a weaker mechanism (no stop, merge waits). The "comparison once more before the merge" condition
  had no occasion: the redraw had no pull request.

### A3. Ask 28 (2026-11-03 00:28), item #205 Fix history gathering

- Who: `by: owner`, "the owner confirmed" (no mention of the delegate's explanation).
- Approved, in translation: "approve the whole design: G3 after T3, T4 counting a change that walks a dropped path again as
  a finding, the four identical gatherer openings made one rule, and paths that no longer exist marked. The owner knows the
  gap G3 fills has no instance yet; it is fixed in the memory system's design; each work gets one more round of the lowest
  level librarian."
- Merged as PR 28 (2026-11-03 02:30 UTC) and at START (cell G3 and the T4 rule "A change that goes back over an approach
  the history of the changed files records as dropped, or against an owner's decision recorded there, is a finding").
- Main now: G3 is gone, and so is the finding rule: no cell of the engineer graph has a sentence about a change that goes
  back over a dropped approach. The review cells (T2, T3) get "The background" (R0, gathered once at the start) and
  "Decisions in force"; the history can still be asked for with `history`. The path rule stays: "A path in the history is the
  path of the day it was written. Before you hand a brief in, check every file or directory path it names..." (identical in
  START, END and NOW), and the four openings are one rule (`rules.2`).
- Later owner words: none found.
- Verdict: two of the four parts removed (G3, the finding); two kept.

### A4. Ask 31 (2026-11-03 02:44), item #228 Merge similar reports

- Who: `by: owner`, answer "OK" in translation: "Fine."
- Approved: the architect answers side reports and engineers' answers about one thing in one round, a lead matter and the
  others (at most 8 in a group), written asymmetrically (a lead plus the rest), with the check that every one is answered.
- Merged as PR 31 (2026-11-03 05:21 UTC); at START: the picking cell's `with` field (at most 7), the check `picked_pickable`
  and `group_answered`, the action `close_matter` closing a group, the owner role's `dissolve_group`.
- Main now: none of it. Q1 is gone; `close_matter` closes one matter; the checks `picked_pickable` and `group_answered` are
  gone; `dissolve_group` is gone from the owner role. Each matter has a session of its own forked from the base.
- Later owner words: none found.
- Verdict: removed.

### A5. Asks 25, 26, 30 (2026-11-03 00:28, 00:28, 02:11), item #195 Record owner decisions

- Who: `by: owner`; 25 and 26 "the owner confirmed"; 30 "the owner confirmed after reading the delegate's plain
  explanation".
- Approved, in translation:
  - Ask 25: the field `speaker` is required; and the design changes so the delegate can mark which part of a message is
    the owner's own words, which counts as the owner's, the decision line saying it was relayed by the delegate; the engine
    cannot check the delegate's chat, and where it cannot it must say so.
  - Ask 26: rename the head `Owner's decisions:` to `Decisions:` and change the head of 182 items by a migration, "each
    line's 'Decision (date, source)' says who decided, no information is lost, one form to maintain".
  - Ask 30: approve the ten wordings in the flow: the readers of history get a source `owner relayed by the delegate, not
    checked`, counted as the owner's decision but with the note that nobody checked what the owner said; recording quotes
    only the marked paragraph.
- Merged as PR 16 (2026-11-03 03:11 UTC) and at START. At START the architect's cells B5 "Record the owner's decisions" and
  H12 "What was said in the groups", and the engineer's L2, take decisions and write them as lines in the item.
- Main now: `speaker` is still a required string in the owner commands (`crates/wf-agents/src/main.rs`), and the source
  `owner relayed by the delegate, not checked` is still in the history readers' rule. But the architect's rule at END says
  "No decision line is written in an item: a design decision is made in a work's deep research, chosen by the owner, and kept
  in the project's `decisions/`. What a talk with the owner settled for `decisions/` goes into the item's decision points";
  B5, H12 and the checks `decision_words_hold`, `words_all_looked` are gone; the shape `decision` (the owner's words become a
  line of an item) is still defined but no `shape` or `$ref` field refers to it at END (three did at START). The existing lines stay "letter for letter as history". The head `Decisions:` is
  still in the history readers' text.
- Later owner words: none on this; the `decisions/` directory that took the place of the item lines came with commit `7f3d27a5` (a
  direct push, 2026-11-04 05:39 PST).
- Verdict: the approval about recording is replaced; the approvals on `speaker` and on the source label stay in part.

### A6. Ask 35 (2026-11-03 03:36), item #219 Drop item dependencies

- Who: `by: owner`, "the owner confirmed after reading the delegate's plain explanation".
- Approved: add `remove_dependency` to the item changes, a pair of `add_dependency`, naming the item, the item no longer
  waited for and why; "it has to pass the batch review", the delete button on the page stays.
- Merged as PR 34 (2026-11-03 04:58 UTC) and at START.
- Main now: `remove_dependency` is still in the item change shape. The batch review (B1 to B3 and the check
  `review_covers_changes`) is gone. Its place is the second eye (G1, G2), which looks only at a cancel, a merge, a split, a
  change that rewrites or removes an acceptance, or a change on an item a work runs on; any other item change, a dependency
  removal included, is made when the round ends.
- Later owner words: none found.
- Verdict: changed: the removal stays, the review "like adding one" is no longer there for either.

### A7. Ask 41 (2026-11-03 06:01), item #16 Fix review and proofs

- Who: `by: owner`, "agreed, load as a column of its own (a) (the owner decided by the delegate's advice)".
- Approved, in translation, four things: (1) T1, T3 and T4 get "what changed on the branch since last time", T3 also the
  CI result of the head; (2) the sentence "the re-check answers all columns" made to match the engine's check; (3) the
  condition table gets two new ways to prove, a CI job or reading the diff; (4) the design review (D3) and the
  implementation review (T4) both answer how the cost of the code grows when load doubles, per event, per round and per
  work; as a column of its own in the T4 table.
- Merged as PR 39 (2026-11-03 08:50 UTC) and at START (rule "Cost under load" in the engineer's rules).
- Main now: (3) is kept: the implementation's conditions have `ci_job` and `read_diff`. (4) is gone: no text of the flow
  asks about cost under load (no match for "under load" or "hot path" at END or NOW), the T4 table is gone with the check
  `columns_answered`. (1) is partly kept: T1's input "What the branch did since you last answered" is gone; the review
  cells T2, T3, T4 get "What the review reviews", and T4's says "the commits the change is read by, with the head the review
  last looked at"; the CI result is not a T3 input any more ("The engineer's tests and their runs" is). (2) went with the
  table.
- Later owner words: none found.
- Verdict: changed (3 kept, 4 removed, 1 partly, 2 void).

### A8. Ask 39 (2026-11-03 04:45), item #217 Fix return forms

- Who: `by: owner`, "OK".
- Approved: five wordings: (1) the design's "measured or not" column gets a way to say "deduced from reading the records";
  (2) the engineer writes a test once, the other two tables are worked out by the engine; (3) write when a locked
  acceptance changes and who can change it; (4) say which run decides a test's result before the change; (5) item and
  observation titles in the owner's language, width counted by display.
- Merged as PR 36 (2026-11-03 08:17 UTC) and at START.
- Main now: (2) kept (the engineer lists `test_changes` once; the engine reads the results from the runs); (4) kept ("the
  latest run of `ci_before` that reported the test counting"); (5) kept (`items.title_max` in the item change and the
  observation titles); (3) changed: no text at END or NOW holds "locked" for an acceptance, the check
  `acceptance_matches_locked` ("Locked acceptance conditions are handed in word for word; to change them, the agent asks the
  advisor first") is gone, and the engineer changes the acceptance itself by handing in the whole list as `acceptance_changed`
  with why, for the review to judge; (1) not traced: the design's columns and the gates D4 to D11 are gone, and the new
  design form was not read for this.
- Later owner words: none found.
- Verdict: changed in (3), (1) unknown.

## 5. Approvals that stay

Same JSON value at START and END, unless said:

| Ask | Item | Approved | Evidence |
| --- | ---- | -------- | -------- |
| 27 | #20 | `stopped` row of the merge cell M2; cancel and pause behaviour | M2 still has `stopped`; it gained `third_failure`; engine behaviour not diffed |
| 29 | #226 | Counters kept per round | views `work_counters`, `work_asking` unchanged in `db/schema.sql` |
| 34 | #221 | Merge queue ordered by arrival at the merge cell | view `merge_queue` unchanged |
| 36 | #229 | Questions to the owner written by five points; a question is withdrawn only when the cell it goes back to hears the words | the owner role's text equal but for one dropped `do`; `asking.handOver`, `receive`, `counts` equal |
| 42 | #233 | `gh` and `measure` described as the code runs them | the descriptions of `gh` and `measure` equal; the check `measure_matches_runs` stays |
| 48 | #239 | Who may lift which hold; Pause all queues no free-slot matter | `engine.stores.holds`, the action `release_starts`, cell S0's holds input and its cases equal |
| 49 | #230 | Next round runs the new version; the work is brought to it through V1 | V1 in both graphs; the text of `bring_up` extended; the "new input to the session or redo the cell" part not traced |
| 25 | #195 | `speaker` required | `speaker: String` in the owner commands |
| 38 | #217 | Try `claude -p` | not about the flow |

## 6. Approvals that never reached main, or are not about the flow

- Asks 37, 43, 45 (#236 handing a long round to a new session): PR 37 was closed without a merge; there is no handover cell
  at START, END or NOW. Nothing was removed.
- Ask 44 (#251 Run both CIs together, two sentences in the flow): no pull request for it in the list; no such sentence at
  START or END.
- Asks 24, 33, 46 (policy approved) and ask 32 (policy half-sentence), ask 23 (the 11-01 candidate-flow instruction no
  longer holds): the policy and an instruction, not the flow. The policy text was not read for this report.
- Asks 2 to 19 (before 11-02) were read: they are policy approvals, small-item choices and engine answers; none approves
  flow content.

## 7. After the window

- Asks 50 to 79 were answered after the window, so no push of the window changed them. Several say the redraw
  replaced their subjects: ask 54 (#276 cancelled, "replaced by the new engineer graph"), ask 59 ("the old review table and
  its columns no longer exist"), ask 65.
- Direct pushes after the window, not in the table: `fe319ae9` (2026-11-04 15:01 PST) "Go to deep research only when the
  research cannot settle the approach, and let the work go on past every approval it asks", and `ece3499a` (14:24 PST) "Have
  the owner approve a change to the flow at the merge, not as an open question of a design". The first changes the 11-03
  "stop and confirm before implementing" that ask 77 then replaced; the second changes where the owner approves a flow
  change (AGENTS.md says the same today). They are outside the window and were not examined for other approvals.

## 8. Records read and not read

Read: owner_asks 2 to 79 (asks 25 to 49 in full, the others cut); the three flow files (START, END, NOW) by cell, by field
and by phrase; `engine.checks` of START and END; `git log` of main for the window and for flow.json since 11-02; the list of
pull requests; the schema diff for the views named above.

Not read:
- Words of the owner that are only in the delegate's chat, never in `owner_asks`. Ask 25 says the database does not hold them.
- The decision cards (`design_cards`) and their rulings, `owner_messages`, `interjections`: the approvals of the designs that
  gave the flow changes. Asks 25 to 49 are the asks of the owner for flow changes; the designs' own cards were not queried.
- The decisions of items (the `Decision (` lines) and the policy's principles; the policy text.
- The diffs of the 97 commits; only the flow file's cells, fields and checks and the schema views were compared. Changes to
  Rust code, tests and `db/schema.sql` other than named above were not compared with the approvals.
- Whether any approval's rider, such as "send me the flow page's comparison before the merge", was met for the pull requests
  before the window.
- `decisions/`: the six files given as input were read; none states an approval removed by the redraw. The brief counted
  eight files; the other two were not read.

## 9. Reproduce

1. `git worktree add --detach <dir> a5143988` and the same for `0ecb0d81` and `9228c1f7`.
2. List the cells: `jq -r '.flows[] | .name as $n | .nodes[] | [$n, .id, .kind, .title] | @tsv' <dir>/flow/flow.json`.
3. Equality of one part: `jq -s '.[0].shapes.arch_action.branches[4] == .[1].shapes.arch_action.branches[4]' <start> <end>`.
4. Phrases: `jq -r '[.. | strings] as $s | $ARGS.positional[] as $p | "\([$s[] | select(contains($p))] | length)\t\($p)"' <file> --args 'under load'`.
   `probe.jq` is the same count written for a file.
5. The asks: `SELECT id, work, round, body, answer FROM owner_asks WHERE id BETWEEN 2 AND 79`.
6. The pull requests: `gh api repos/Ming-Research/wf-agents/pulls?state=all&per_page=100`.
