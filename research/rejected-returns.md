# Returns the engine sent back since flow version 33

For: the architect and the owner, who decide the fix for each class below (item #198, #212, #213 and the rest).
Kept: as a dated record until the fixes have landed and its numbers are out of date; then deleted.
Written: 2026-11-02, item #197, work 247. Data snapshot: 2026-11-02 19:50 UTC (round R247-37).

How to read the claims. Each carries a basis: **[read]** a source file, a stored return or the flow text was read in
this work; **[measured]** counted by a database query given in this report; **[reproduced]** a command was run here;
**[inferred]** deduced, with how to confirm it. Where a message of an agent is described, it is paraphrased in English,
not quoted.

## 1. Question

Item #197 asks why the engine sends returns back, which causes lie in a cell's text, an input, a tool or the check
itself rather than in the model's own judgment, and what generic fix each class needs. The owner's goal is that no
return is sent back. This report classifies every return sent back since flow version 33, covers the five of item
#192, and decides whether the check `evidence_holds` is still needed. It changes nothing.

## 2. Scope and method

Scope. A round whose `rounds.rejected` holds a `check` or a `format` key and which started at or after flow version
33 was recorded (1793622870365 ms, 2026-11-02 12:34 UTC). Not counted: failed runs, interrupted runs and `error`
rows, which are not returns: there are none since version 33; in all history there are 32 failed, 10 interrupted and 6
error rows [measured]. All rows with `rejected` set in the database: 146 = 68 check + 30 format + 32 failed + 10
interrupted + 6 error; 98 of them are returns sent back, 31 since version 33 [measured at R247-33 for the older rows,
with the two sent back since added].

Snapshot. 31 rows: 24 check and 7 format; the last is R241-41. The earlier research count of 27 (R247-11) was taken
before R241-28, R241-35, R241-40 and R241-41 were sent back; the table grew while this report was written, and
three more came after the snapshot (section 4b); more may have appeared since. The one query that gives the table of
section 4 as it stood at the snapshot (its row count is the 31, and 34 at R247-40):

```sql
-- Returns the engine sent back since flow version 33 was recorded.
SELECT r.work, r.n, r.cell, r.started_at, r.rejected
FROM rounds r
WHERE r.rejected IS NOT NULL
  AND r.started_at >= 1793622870365
  AND (json_extract(r.rejected, '$.check') IS NOT NULL
       OR json_extract(r.rejected, '$.format') IS NOT NULL)
ORDER BY r.work, r.n;
```

The works of the rows ran on flow version 33 and none had moved from another version (checked when the table had 28
rows; the three later rows are all in work 241, which was among the works checked) [measured]. All 31
rounds ran on the engine build 0f7dc28 [measured, `rounds.input` call].

Sources read. The engine and flow were read at repository commit 6a3eeea. The last commit that changed
`flow/flow.json` or `crates/engine/src` before version 33 was recorded is be10fd2 (2026-11-02 12:09 UTC), and none
changed them after, up to 6a3eeea, which contains the build 0f7dc28 [read, `git log`], so the checks and texts quoted
below are those the 31 rounds met.

Rule for the classes. Each round gets one class: the one problem that would still get the return sent back after the
others in it are taken away. Judged in this order:

- **a** The return is right by the rules the model saw; the check (or a check left over from an earlier time) is wrong.
- **b** The return is valid under the schema the command line was given and invalid under the cell's own schema.
- **c** The cell's text or inputs leave the rule unsaid, say it the wrong way, contradict each other or the engine, or
  lack a value the rule needs.
- **d** The text states the rule and the model did not follow it (counting characters, keeping to a stated form).
- **e** The return reports something false or inconsistent about the work itself; the check was right.

Approaches compared (design of this work). Reading each return together with the check's or schema's source (chosen);
replaying all stored returns mechanically through both schemas and every check (needs a program that reads the
database; the sessions can only run read-only queries); widening to all 98 returns of all versions (older flows
differ, each would need rebuilding first); making the engine store a category first and querying it (a code change,
and a check's name does not give the cause: R227-22 and R238-34 are the same check with different causes).

Not recorded anywhere: the rejections the command line makes inside one run and fixes by its own retries
(`MAX_STRUCTURED_OUTPUT_RETRIES` is 3, `crates/sessions/src/cli.rs:104`). They cost nothing the engine sees, so they
are not in this table [read].

## 3. Summary

| Class | Returns | Share | Cause lies in | Round cost (sum) |
|---|---|---|---|---|
| a. check wrong | 10 | 32% | `conditions_cover_acceptance` (5), `evidence_holds` (5) | $2.00 |
| b. command-line schema looser than the cell's | 7 | 23% | the merged schema the command line gets | $1.82 |
| c. text or input gap | 10 | 32% | D2, T4 texts, the I1 and T6 test tables, the architect cells' inputs | $3.20 |
| d. model did not follow a stated rule | 2 | 6% | the model | $0.19 |
| e. check was right | 2 | 6% | the returns | $0.18 |

The shares are rounded. 27 of the 31 (87%) have their cause in the system, and a fix there removes them; 4 are the
model's or the work's own. By cell: T4 8, I1 7, T1 4, D2 3, T6 3, and one each for T3, H8, H5, H3, G1, D3 [measured].

## 4. The 31 returns

Round address is `R<work>-<n>`. "Message" paraphrases what the engine said.

| Round | Cell | Sent back by | Message | Class |
|---|---|---|---|---|
| R226-31 | H8 | `effects_valid` | a merge into an observation would make it reported 3 times, which must be folded into an item | c |
| R227-21 | I1 | format | `deviations[3]` has `why`, not a field of its kind | b |
| R227-22 | I1 | `conditions_cover_acceptance` | a vitest test named as describe > it is not on the branch | a |
| R227-26 | T3 | format | `tools[0].tool` is "query", must be "gh" | b |
| R227-30 | T4 | `evidence_holds` (2 evidence objects) | two `nodes` entries that are round addresses | a |
| R227-40 | T4 | `no_adversarial_when_due` | at a re-review `no_adversarial` is only `does_not_stand` | d |
| R228-12 | H3 | `effects_valid` | a merge would make an observation reported 6 times | c |
| R231-21 | D2 | `measure_matches_runs` | answered measured, but the work has no measure run | c |
| R231-76 | T1 | `conditions_cover_acceptance` | the "test" `npm run check:ui (in the lint job, green)` is not on the branch | a |
| R231-102 | T4 | `evidence_holds` (2) | two `nodes` entries that are sentences (a design answer, a PR description) | a |
| R232-40 | T1 | `test_kinds_match` | a guard test written by the adversarial tester has a result before the change that does not fit its table | e |
| R232-45 | D2 | `acceptance_matches_locked` | locked acceptance must be handed in word for word; ask the advisor first to change it | c |
| R232-78 | T4 | `evidence_holds` (2) | two `nodes` entries that are round addresses with a sentence | a |
| R233-59 | T4 | `columns_answered` | column 1 not answered (19 listed, 9 answered) | c |
| R234-23 | I1 | `conditions_cover_acceptance` | vitest name (describe > it) not on the branch | a |
| R234-26 | I1 | `conditions_cover_acceptance` | the same, reworded | a |
| R234-27 | I1 | `new_test_results` | a test added as new behaviour is not in the `new_tests` table | c |
| R234-34 | T4 | `evidence_holds` | a `nodes` entry that is a round address with a word | a |
| R238-23 | I1 | format | `deviations[2]` has `why` | b |
| R238-34 | T1 | `conditions_cover_acceptance`, `adversarial_test_results` (2) | nextest-style name not found; a test said red on the head where CI is green; tests written but head not pushed with ci | e |
| R238-37 | T1 | `conditions_cover_acceptance` | nextest-style name (`engine::e2e name`) not on the branch | a |
| R238-58 | T6 | format | `deviations[3]` has `why` | b |
| R238-64 | T4 | `columns_answered` | column 1 not answered (27 listed, 8 answered) | c |
| R239-53 | H5 | `effects_valid` | two observation titles of 22 characters, 20 at most | d |
| R241-2 | G1 | format | `outcome` holds `brief` and `ask` (null) and fits none of its forms | b |
| R241-28 | I1 | `new_test_results` | an existing test listed as added new behaviour is not in the `new_tests` table | c |
| R241-35 | T4 | `evidence_holds` | a `files` entry starting with a bare file name is not in the worktree | a |
| R241-40 | T6 | format | `deviations[2]` has `why` | b |
| R241-41 | T6 | `new_test_results` | a test said not to have run before the change, where ci_before reports it failed | c |
| R242-13 | D2 | `measure_matches_runs` | answered measured, but the work has no measure run | c |
| R242-15 | D3 | format | `better` has `considered`, not a field of `found` | b |

Counts: a 10, b 7, c 10, d 2, e 2 = 31 [measured, this table is the query's rows].

## 4b. Sent back after the snapshot (not in the totals of this report)

By R247-40 the query of section 2 returned 34 rows; the three beyond the 31 above were read the same way. With them the
classes are a 12, b 8, c 10, d 2, e 2. Their own cost is not added to section 8 [measured].

| Round | Cell | Sent back by | Message | Class | Cost |
|---|---|---|---|---|---|
| R242-38 | D7 | `answers_match_speakers` (`redirect.answers.to`) | the architect did not interject in this round | a | $0.07, 19 s |
| R246-29 | T3 | format | `tools[0].tool` is "query", must be "gh" | b | $0.45, 177 s |
| R247-38 | I1 | `answers_match_speakers` (`answers.to`) | the architect did not interject in this round | a | $0.75, 157 s |

- `answers_match_speakers` (`checks.rs:961-977`) accepts an answer only to a side that interjected in the same round
  [read]. In R247-38 (this work) the architect's interjection ij:87 had been delivered in the round before, R247-36
  (`interjections.delivered_round` is 36), and R247-38 was given none (its `engine.interjections` is empty); for
  R242-38 the architect's ij:90 was delivered in R242-37 and R242-38 was given none [measured]. The interjection was shown to
  R247-38 again as a hook note that said to answer it in that round's return [read, this work's prompt]. R247-36 had
  ended in a request for tools, a form that holds no answers, so the interjection could not be answered there, and
  the round that was told to answer it was refused for answering it. The model did what the text said and the check
  looked at one round, so this is class a. That R242-38 saw the same note is inferred from its answer to the architect
  [inferred; confirm by reading its stored message]. Fix: an interjection is answerable in the round that
  was told to answer it, or is not shown again after its round; or the `tools` form can carry answers.
- R246-29 repeats R227-26 (class b) in another work: T3 asked, with the `query` tool it does not have, for its work's CI
  record and an item row [read, its stored return]. Twice since version 33, T3 reached for the CI results it is not
  given. Giving T3 the CI results as an input removes the call and the lost round in both cases.

## 5. Classes

### a. The check is wrong (10): R227-22, R227-30, R231-76, R231-102, R232-78, R234-23, R234-26, R234-34, R238-37, R241-35

**`conditions_cover_acceptance` (5: R227-22, R231-76, R234-23, R234-26, R238-37).** Where: `crates/engine/src/checks.rs`
lines 524-531. For each test an acceptance row lists, the check keeps the part after the last `:`, `/` or `.`, trims
it and runs `git grep -q -F -w` for that string on HEAD; no hit means "the test is not on the branch" [read]. The model
writes the name the way its test runner prints it, and the check searches a string no source file holds:

- vitest `describe > it` (R227-22, R234-23, R234-26). The two halves sit on different lines of the source. In this
  tree the whole name `a diagram wider than its box > is scaled down to the box in proportion and never scrolls in it`
  matches no file, the `it` title alone matches `client/src/diagram.test.tsx`, and the `describe` half is on line
  35 beside the `it` on line 36 [reproduced: `git grep -c -F -w` on HEAD].
- nextest `binary name` (R238-37; also R238-34, counted under e). The check cuts at `::` and keeps `e2e name` or
  `flow name`, which no file holds; the bare function name does (`building_a_message_writes_nothing`: 0 files with the
  `e2e ` prefix, 1 file without it) [reproduced]. The tests exist: the ci tool recorded
  `engine::e2e an_open_question_pinned_for_the_owner_holds_the_merge_until_the_owner_has_answered` as passed at
  rounds 22, 27, 36, 40, 46 and 55 of work 238, and R238-38 was accepted with the bare names [measured].
- a CI step among the tests (R231-76): `npm run check:ui (in the lint job, green)`; the kept part
  `ui (in the lint job, green)` matches no file [reproduced]. The step is real: the project's own rule names
  `check:ui`; it is not a test name, which the field `tests` asks for.

Root cause: the check guesses a search word from a display string, with no oracle of its own. The engine already
holds the oracle: the ci and ci_before results name the tests that ran. One limit: the ci tool reads no per-test
result of vitest; R227-21's own deviation reports all eight vitest names as not run although the job was green [read].

Generic fix (owned by #212 and #213; recorded here only): a test is identified by one id per test runner, kept where the
runner is described (the project's configuration), and the engine compares ids with the runs it recorded or with a
search token the configuration defines for that runner; for a runner whose per-test result the engine cannot read, the
check says so and accepts a named CI job step as evidence. Trade-off: a per-runner definition is more to configure than
one grep, but it removes the guess.

**`evidence_holds` (5: R227-30, R231-102, R232-78, R234-34, R241-35).** See section 7; the verdict there is to remove
the check.

### b. The command line's schema is looser than the cell's (7): R227-21, R227-26, R238-23, R238-58, R241-2, R241-40, R242-15

Cause: one return is described three times and the descriptions differ [read]:

1. The schema given to the command line (`--json-schema`) is one schema for the whole line of cells, built by
   merging the shapes of every cell's forms (`envelope`, `shape`, `merged`, `cli.rs:722-1069`); it carries types and
   fields, no discriminator values. Stored for R227-21 (contents hash 94cc52b9, 9994 characters): `deviations.items`
   is an object with the strings `kind`, `what`, `why`, requiring `kind` and `what`, closed. A line-wide schema is used so
   that it does not change from round to round, which would leave nothing of the session's prompt cache
   (`cli.rs:693-696`).
2. The cell's schema, shown in the message as `format:<cell>`: a `oneOf` by `kind`; `surprise` has `kind` and `what`
   only, `workaround` and `design_changed` add `why` (`flow/flow.json:210-269`).
3. The engine's validation against the cell's schema, which treats each form as closed.

A return that the first accepts and the second refuses is sent back by the engine as a whole round. The command line
retries up to 3 times inside one run when its own schema refuses (`cli.rs:104`); none of these seven reached that path
because its schema accepted them [read].

- `deviations` (R227-21 entries 3 and 4, R238-23 entries 2 to 4, R238-58 entry 3, R241-40 entry 2, counted from 0): the
  refused entries are all `surprise` rows with a real `why` text [read, stored returns]. R241-40 is the T6 round of
  work 241; its next return (R241-41) dropped the `why` and was sent back for another reason (class c).
- `better` (R242-15): the return is `found` and carries `considered`, which belongs to `present_best`
  (`flow.json:5787-5826`) [read].
- `tools` (R227-26): T3 may use `gh` only (`flow.json:7665-7667`); the command line's schema has `tool` as any string; the
  return asked to run a read-only query on the CI table [read]. T3's inputs hold no CI result
  (`flow.json:7668-7714`), which is why it wanted one; the item treats that as T3's own feedback and it is not
  counted here beyond causing the call.
- `ask` null (R241-2): the return had `brief` (text) and `ask: null` [read, `json_type`]. The command line allows
  null for a field its object may leave out (`or_null`, `cli.rs:986`). The engine drops a null only for a field of
  the form the value fits (`without_null_options`, `crates/flow/src/compiled.rs:573-608`); `ask` is not a field of
  the `brief` form, so it stays and the value fits no form. The next round R241-3 handed in `brief` alone [read].
  That the null survives is deduced from reading that function, not by running it [inferred; confirm with a unit test
  that feeds this value to `valid` after `without_null_options`].

Generic fix directions, with trade-offs:

1. Build both schemas from the one compiled union. Keep a discriminated `oneOf` (const `kind`) below the top level of
   the line's schema and merge only where the API forbids it (the top level; `cli.rs:801-805`). Gain: the command line
   refuses `why` on `surprise` and `considered` on `found` inside the run, which is a free retry instead of a lost
   round. Cost: a larger schema, still one per line so the cache argument holds. Risk: nested `oneOf` support in the
   command line is unverified [inferred; confirm with one trial call, as the code comment did for the top level].
2. Make the cells' schemas accept what the loose schema accepts where it is harmless: a `why` on every kind of
   deviation; a null for any optional field of an object at any depth, not only of the matched form. Gain: no round
   lost. Cost: the cell's description gets longer, and an agent's extra field is kept or silently dropped.
3. Keep both and say in each cell which fields belong to which form (it already does in `desc`); this was the state in
   all seven rounds and did not prevent them.

R227-26 (the `tools` list) is different. The allowed tools differ by cell and the schema is per line: T3 and T4 are both
on the line `review`, T3 with `gh` only and T4 with `gh` and `query` (`flow.json:7664-7667`, `:7794-7798`) [read], so
a line-wide schema has to allow `query` for T3 too. None of the three directions can make the command line refuse it;
only a schema per cell could, which gives up the shared prompt cache. The engine's refusal stays the guard there, and
the cheaper fix is on the cell side: give T3 what it wanted (the CI results) so it does not reach for a tool it lacks.

### c. The cell's text or input has a gap (10)

**c1. The measure answer (R231-21, R242-13; `measure_matches_runs`, `checks.rs:333-341`).** D2's `measure` has two
answers: `not_measured` ("could be decided without measuring") and `measured` ("decided by the measurements you asked
for before"), `flow.json:5574-5604`. The check counts only runs of the `measure` tool. Work 231 had 67 query runs, 17
ci, 5 ci_before, 2 gh, 1 full_ci and 0 measure; work 242 had 12 query runs and 0 measure [measured]. Both returns
describe a replay or a read of recorded rounds by read-only queries as "the measurement" [read, their `effect`
texts]. The model's reading is natural; neither answer fits "I read recorded data". Fix: a third answer, "decided from
recorded data read with queries" with what was read; or word `measured` as "the measure tool ran"; the check stays as it is.

**c2. Locked acceptance (R232-45; `acceptance_matches_locked`, `checks.rs:421-439`).** The architect changed the item's
own acceptance text; the lock holds the earlier text, and D2's input says "locked, including what the advisor
changed" (`flow.json:5523-5524`). D2's field text names only the advisor for a change (`flow.json:5642`). The model's
own flow feedback on that round (paraphrased) says it knew the rule, saw the lock still held the old text, and wrote
the architect's wording because the architect outranks the advisor [read]. The retry text asks for "the kind
acceptance_change", a word that appears only in two retry texts (`checks.rs:435`, `:513`) and in no form of the flow; R232-46 put
"kind: acceptance_change." at the start of its ask's text instead [read, grep of the repository]. Fix: say in the input
that the lock changes only through the advisor's reply and that a change of the item's text by the architect or
owner does not move it; give the ask a field if the engine needs the kind, or drop the word. The item names D2 and T6;
only D2 was read here.

**c3. Columns at a re-review (R233-59, R238-64; `columns_answered`, `checks.rs:736-752`).** The engine listed 19 and
27 columns and the model answered 9 (ids 2,3,5,6,7,10,11,12,18) and 8 (ids 2,3,6,7,9,10,13,22); the next rounds
answered all 19 and all 27 [measured]. T4's text and the field text say that at a re-review the engine lists only the
columns the changed part concerns (`flow.json:4146` and `:8131`), while the engine's code lists them all, saying so
in its comment: "which columns a change concerns is not worked out in this version"
(`crates/engine/src/message.rs:967-970`) [read]. The text states what the engine does not do. That this made the model skip
columns is inferred from the shape of the answers (the subsets look like the "concerned" columns) [inferred; confirm
by asking the model, or by a trial where the engine lists only a subset]. Fix: make the text true (implement the
selection, as Article 7 asks the flow to say what the engine does) or reword it ("all columns are listed; answer each").

**c4. The same tests in two lists (R234-27, R241-28; `new_test_results`, `checks.rs:534-570`).** I1 writes a test
three times: `test_changes[].test`, the `new_tests` / `guard_tests` table, and the acceptance `tests`; each field is
described as "the test's name", and the checks compare them by different rules (exact string for the first two, a
last-segment grep for the third). R234-27: `test_changes` had `describe > it`, `new_tests` the `it` title alone;
it followed R234-26, where the same model had just been told the `describe > it` form is not found (class a), so it
changed some lists and not the other [read; the acceptance rows, which passed this time, were not read]. R241-28: `test_changes` listed `every_sample_takes_its_row (a sample row ... )` as
an added new-behaviour test; that test already exists (`crates/flow/tests/flow.rs:1020`), the change added a sample
row to its data file, `new_tests` had six other rows [read]. The form for `test_changes` has `added`, `removed`,
`skipped`, `relaxed` and no way to say "an existing test now covers more" [read, format:I1]. Fix: one list, the tables
derived from it (or the other way), so there is nothing to reconcile; add a kind for a test whose cases changed.

**c5. Observation merges (R226-31, R228-12; `effects_valid`).** A merge that makes an observation reported 3 times or
more must be folded into an item (`architect.rs:3172-3192`, `RECURS` is 3 at line 201). At those two moments
observation 84 had been reported 2 times and observation 17 five times, so the merges made 3 and 6 [measured,
`changes`]. Neither cell is given the number: no input of R226-31 holds an observation entry (20 inputs, none about
observations; no text they refer to holds the key of observation 84), and the candidate list R228-11 handed to
R228-12 holds only an id and a reason per observation [read; the negative search covers every text the round's input
refers to]. `observations_overview` (`architect.rs:4346-4396`) gives the count for the ten most recently updated and
is not among these cells' inputs [read]. The rule's text is `flow.json:3164`; whether it is in the cell's message was
not checked. Class c because the rule needs a value the cell cannot see [inferred as to the cell's inputs; confirm by
reading the two stored messages in full]. Fix: show the number of reports with every observation a cell may name (in the
search result and the matter), or let the engine do the fold step the rule demands when the count reaches 3.

**c6. A test changed after its last ci_before run (R241-41; `new_test_results`).** T6 handed in the result before the
change of `every_answer_to_a_row_a_side_raised_makes_that_side_look_again` as `not_run`; the engine said ci_before
reports it failed [read, retry text and return]. The test had been run through ci_before earlier in the same work and
was red before the change (the table review of R241-35 cites that run, round 25 of work 241) [read]. T6's own deviation
says it extended the test in this round and did not run the extended version on the old code, so it handed in `not_run`
[read, paraphrased]. The text for the table says to hand in the two results "as those tools report them", for the tests
added or changed in this round; it does not say what to hand in for a test that changed after its last ci_before run,
and the engine matches by name, so the earlier version's result counts [read; the matching code, `test_results_match_runs`,
was not read: inferred from the retry text]. The reading T6 took is reasoned, so this is class c; it would be class
e if "as the tools report them" is read strictly. Fix: say which run counts; or key the engine's match by the test and
the head it ran on; or have the engine fill the result column from its own records, so the agent hands in names only.

### d. The model did not follow a stated rule (2): R227-40, R239-53

- R227-40, `no_adversarial_when_due` (`checks.rs:821-838`). The first table review that was accepted was R227-31
  (R227-28 asked for tools, R227-30 was sent back for evidence); it handed in `stands`. The re-review R227-40 handed in
  `stands` again and was sent back; R227-38 and R227-41 left the field out [measured, transitions of work 227]. The field
  text says it is handed in at a re-review only when the judgment becomes `does_not_stand` (`flow.json:8215`). Repeating
  a verdict that has not changed does no harm. Fix options: accept a repeated `stands`, which drops the refusal; or
  make the first review's judgment stand until a return changes it, so the model has no decision to remember. Whether
  anything downstream reads a repeated `stands` was not checked [inferred; confirm in `checks.rs` and the merge gate].
- R239-53, `effects_valid`. Two new observation titles, "Acceptance swap unstated" and "Tool rounds lack progress",
  have 22 characters each without spaces (10+4+8 and 4+6+4+8), limit 20; the limit is in the field text
  (`flow.json:1759`) and counted by `arch_changes.rs:130-150` [read, counted by hand]. Fix: put the limit where the
  command line can enforce it inside the run, for example a schema `pattern` that counts non-space characters
  [inferred; support for `pattern` in the command line's structured output is not verified, confirm with one trial
  call].

### e. The check was right (2): R232-40, R238-34

- R232-40. The adversarial tester put a test whose result before the change was `cannot_run` ("what it tests does not
  exist before the change: it tests new behaviour", format:T1's own words for that value) into the guard table; the
  check refused it, the next round R232-41 moved on [read]. The return was inconsistent.
- R238-34. Besides the class-a name problem (counted once, here under the real ones), the return said a test was red
  on the head while the CI runs it was given were green, and said tests were written at head 834a8f3 that were not
  pushed with ci; the ci runs of work 238 in rounds 31 and 33 are on 995bbd7 [read, retry text and tool results]. The
  code of `test_results_match_runs` was not read here [inferred as to the first of the two; confirm by reading it and
  the tool runs of rounds 30-34].

No fix is needed; the checks did their work. One observation: a value offered by the schema that can never pass the
check (`cannot_run` for a guard row) is on purpose, so that a truthful return is possible and the refusal then says
to move the row.

## 6. The five returns of item #192 (work 227)

| Round | Cell | What the item says | Class | Why |
|---|---|---|---|---|
| R227-21 | I1 | the return had an extra field | b | entries 3 and 4 of `deviations` are `surprise` with `why`; the command line's schema allows `why` on any row |
| R227-22 | I1 | acceptance and test names do not match | a | vitest `describe > it` name; the check searches the whole string; the test and both halves are in the source |
| R227-26 | T3 | asked for a tool it does not have | b | `query` is a valid tool string for the command line, not for T3 (`gh` only); cause behind it: T3 is given no CI result |
| R227-30 | T4 | evidence held a round address | a | two `nodes` entries that are round addresses; `evidence_holds` accepts paths only (section 7) |
| R227-40 | T4 | wrong value at a re-review | d | handed in `stands` again at a re-review; the field text says only `does_not_stand` |

## 7. Is `evidence_holds` still needed?

What it does (`checks.rs:753-775`, `path_in` at `:984-991`) [read]: for every evidence object it checks that each `files`
entry whose leading path-like run contains `/` or `.` and does not start with `/` exists in the worktree, and that each
`nodes` entry (after the same cutting and a trailing `/` removed) exists. Its description (`flow.json:3138`) also says
the lines cited are in the diff; the code checks neither lines nor the diff.

Every time it sent a return back (8 returns in all history, 11 entries it names; it stops at the first bad entry of each
evidence object, so there may have been more) [measured, query below]:

| Round | Field | What was refused | A made-up file? |
|---|---|---|---|
| R21-33 | `nodes` | a real path of the design tree followed by a note in brackets; the check of that time took the whole text as the path | no: the file existed; R21-34 cited it alone and passed; the check was fixed in b548d77 (2026-10-30) |
| R28-27 | `files` | a sentence starting with two cell ids joined by `/` | no |
| R29-45 | `files` | a sentence starting with a time (`4.357s ...`) | no |
| R227-30 | `nodes` x2 | round addresses | no |
| R231-102 | `nodes` x2 | a sentence on a design answer; a PR description read through gh | no |
| R232-78 | `nodes` x2 | round addresses with a sentence | no |
| R234-34 | `nodes` | a round address with a word | no |
| R241-35 | `files` | a sentence starting with a bare file name and a running time | no |

```sql
SELECT r.work, r.n, r.cell, substr(json_extract(r.rejected,'$.check'),1,120)
FROM rounds r WHERE instr(r.rejected,'evidence_holds') > 0 ORDER BY r.started_at;
```

Findings:

1. It has never stopped a made-up file. Since version 33 it sent back 5 of the 21 table reviews whose return held an
   evidence object [measured]; all five were evidence that is true but not a path.
2. The `nodes` field is a leftover. It was added on 2026-10-26 (commits 08370fa, the engine reading it, and e265d01, the
   field's text "Which design tree nodes") for pages of the design tree [read, `git log -S`]. The design tree left the
   flow in c223f55 (2026-11-02), there is no `design/` directory, and the field text still gives `design/flow.md` as its
   example (`flow.json:313`) and the title "Files read", the same as `files` (`:288`, `:308`).
3. Only this check reads `nodes` (a search of `crates/` finds `ev.get("nodes")` at `checks.rs:764` and nothing else that reads
   an evidence field); the page's code (`client/src`) reads no evidence field by name [read, grep]. Whether the page
   shows returns generically was not checked [inferred; confirm by reading how the page draws a stored return].
4. In accepted returns `nodes` repeats the paths of `files` without line numbers (R227-30 rows, and all 12 entries
   sampled from R238-65 are real paths) [read, measured sample].
5. The two halves differ in strictness. `nodes` must be paths; `files` is only checked for entries that look like
   paths. Of 596 `files` entries in accepted table reviews since version 33, 28 begin with a word with neither `/` nor
   `.` (approximating `path_in` by the first space-delimited word) and so pass [measured]. The same free text is
   refused in one field and passes in the other, so the check judges which box the model put the evidence in, not whether it
   is true.

Verdict: **not needed; remove the check and the `nodes` field.** What is lost: the engine no longer refuses a cited file
that does not exist. That has not happened once in 11 entries. A cited file that does not exist is found by whoever follows
the citation (the engineer answering the review, the owner). The cost of keeping it was five lost rounds since version
33 ($1.00 of round cost, 5 of 21 reviews); keeping only the `files` half would still have sent back R241-35.
The removal touches `flow/flow.json` and `checks.rs`, so it is an open question to the owner (AGENTS.md); this report
decides, it does not make the change.

## 8. What the returns cost

Snapshot at R247-37: 837 agent rounds since version 33 (rounds with a recorded usage), $237.28, 873.1 minutes of
round time. The minutes are the sum of round durations; rounds of different works overlap, so it is not elapsed time
[measured].

| | Rounds | Cost | Round time |
|---|---|---|---|
| The 31 returns sent back | 31 (3.7% of all rounds) | $7.39 (3.1%) | 47.0 min (5.4%) |
| The round that rewrote each (same work, same cell, next round, not itself sent back) | 28 | $4.51 | 19.6 min |
| Together | 59 | $11.90 (5.0%) | 66.6 min (7.6%) |

The rewrite is an upper bound of the waste, since a rewrite may also do other work (the longest are R242-14 at 171 s,
and R228-13 and R234-28 at 103 s each). Three of the 31 (R227-21, R234-26 and R241-40) have a sent-back round as their next
round (R227-22, R234-27 and R241-41), which is counted once, among the 31. The sent-back rounds by class, from per-round values: a $2.00 and 11.2
min, b $1.82 and 13.0 min, c $3.20 and 20.6 min, d $0.19 and 0.7 min, e $0.18 and 1.3 min (the minutes sum to 46.7, the
difference from 47.0 is the whole seconds kept per row). The dearest: R227-26 ($1.09, 422 s, the T3 call that could not be made), R231-21 ($0.64, 304 s), R234-27
($0.63, 198 s) [measured].

```sql
SELECT count(*) AS n, round(sum(json_extract(usage,'$.cost_usd')),2) AS cost,
       round(sum(ended_at-started_at)/60000.0,1) AS mins,
       sum(rejected IS NOT NULL) AS rejected,
       round(sum(CASE WHEN rejected IS NOT NULL THEN json_extract(usage,'$.cost_usd') END),2) AS rej_cost,
       round(sum(CASE WHEN rejected IS NOT NULL THEN ended_at-started_at END)/60000.0,1) AS rej_mins
FROM rounds WHERE started_at >= 1793622870365 AND usage IS NOT NULL;
```

## 9. Across the classes

- The stored rejection is one string, `check (field): text`. Grouping needs the check name and the field path as
  keys of their own; with them, section 4 would be one query. The class itself stays a human judgment. The command
  line's own retries inside a run are not recorded at all, so the schema's refusals that worked are invisible.
- Where the count is: a and c are 20 of 31. The changes that remove most are: one test identity per runner (a, 5),
  removing `evidence_holds` (a, 5), saying the same thing in the three descriptions of a return (b, 7; direction 1 of
  class b), telling D2, T4, I1 and T6 what the engine does and keeps (c, 10).
- The pause on every rejection (#198) will pause 31 times by this record; 27 of them would be cleared by a fix to the
  system, not by the model's answer. The structured key above would let the pause show the class's usual cause.
- Two causes came back after the snapshot: T3 asking for CI results it is not given (R227-26, R246-29), and an
  interjection that cannot be answered in either round it is shown in (R242-38, R247-38). Both are cheap to remove and
  both recur, which a pause on every rejection would only repeat.

## 10. What is not confirmed

- The model's reasons: c3 (why columns were skipped), c5 (that the cell lacked the count in its message), section 4b
  (that R242-38 saw the note that told it to answer). Confirm by reading the stored messages in full, or by a trial.
- The test names of R234-23 and R234-26 beyond the `describe > it` form: only the form was checked, not whether every
  test exists on work 234's branch.
- `test_results_match_runs` (R238-34, R241-41), what reads a repeated `stands` (R227-40), whether the command line takes a nested
  `oneOf` or a `pattern` (b, d), whether the page draws an evidence object (section 7).
- The class counts depend on the rule in section 2; a different order of the rule (for instance counting every a-class
  problem inside a return) would move R238-34 and R234-27.
- Older versions: the 67 returns sent back before version 33 were not read; their check names are the same, so a
  similar spread is likely [inferred].
