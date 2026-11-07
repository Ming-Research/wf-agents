# I1, T1 and T6 replayed with the test evidence told first

For: the owner, who approves the change to the texts of I1, T1 and T6 and to the checks of their test results at the
merge, and its reviewer.
Kept: while the texts and checks replayed here stand as written; deleted when one of them next changes, git keeping it.
Written: 2026-11-07, on the branch `test-evidence-told-first`, from the trial database as it stood that day.

## 1. Question

Of the 22 returns of I1, T1 and T6 the trial sent back for their test evidence, the causes were: a result before the
change gone stale where running again could not change it (A1); a test the return names that no run of ci on the head
handed in reported (B1); T1's label before the change differing from what ci_before reported (C); a test T6 extended
with no run of ci_before (D). Do the changed texts and checks remove these, and leave the rest of each return as it was?

## 2. What changed

The checks `new_test_results`, `guard_test_results` and `adversarial_test_results` no longer take a result before the
change for stale when the test says `not_before` (or is said `cannot_run`) and the run of ci_before whose result counts
did not build or run it, and no longer blame a test no run reported on the latest run's files (Ch1). The texts the cells
read, each beside its old wording in the pull request: ci's `tests` says only runs on the head handed in count, and T6's
prompt says to name every test its test changes name (Ch2); `on_base` gives the mapping from ci_before's result to the
label (Ch3); `test_changes` says a row added or extended is a test ci reports by name (Ch4); T6's prompt says to run
each test it adds or extends with ci and ci_before (Ch5); `test_name` says a name ends at a test's own name (Ch6); a row
of ci whose name matched no test says why (Ch7); AGENTS.md, the project guide, says how this project names its tests
(Ch8).

## 3. How the rounds were run

A round is rebuilt from `rounds.input` (the message word for word and the whole command line) with the stored contents
put back. Every round replayed here resumed a session, whose transcript is kept in the data directory, which this work
does not read; so each was run as a new session, which the engine also starts when a session's context changes: every
conditional input the session already had is given in full, its text taken from the round that gave it, or, for a field
of an earlier return, that field's JSON. For T1 the tool results of its earlier rounds, which a new session would lack
and the original had in context, are given too. The working directory is a scratch clone at the round's head, on the
work's branch, with no remote, `origin/main` at main as it was when the round started, and every other ref removed; a
new session id, the default Claude configuration directory, no `--settings` hooks, and the `--add-dir` directories
pointed at empty scratch directories. The new texts were put in where the old stood in the message, the system prompt
and the schema, adjusted to the wording each round's flow version had; a run's `not_run` rows with nothing matched were
given the `why` Ch7 adds. Where a round asked for `query`, the query was run by hand on the trial database, limited to
the rows before the round, and given back in the resumed replay session as the engine gives tool results. No `ci` ran.
Cost of all replays: about 7.7 USD.

A new session does not reproduce a resumed one: it reads the diff and the results afresh rather than recalling what it
did. The original's error did not come back in that form in two of the cases (C at T1, B1 at I1), so those show only
that the new texts change nothing there.

## 4. Cases

### A1, a stale result that running again cannot change: the check, on the stored returns

The returns sent back for A1 alone were judged by hand with the changed rule on the runs of ci_before the engine had
recorded when each was handed in.

| Return | The test and what the run that counts said | Old check | Changed check |
| --- | --- | --- | --- |
| R368-28, R368-29 (T6) | `own_rounds_finds_…` says `not_before`; ci_before at R368-24 reported it not built; its file moved since | sent back twice, then the advisor, then a ci_before that reported it not run | taken at R368-28 |
| R329-64 (I1) | three `sent_back_lines::…` tests say `not_before`; ci_before at R329-24 reported them not built; its files changed since | sent back | taken: the other tests' runs (R329-63) were on the head handed in |
| R308-81 (T6) | `a_value_inside_a_column_of_json_…` says `not_before`, in an implementation file; no run of ci_before reported it | sent back for the latest run's files (R308-63) | taken |
| R329-22 (T1) | six tests said `cannot_run`; ci_before at R329-17 reported them not built; a file of that run changed | six of its nine problems | none of those six |

R368-28 replayed as a new session (Haiku, as the original): both texts hand in F1 fixed with no test rows, the same
return, so the texts change nothing there and the change is the check's.

### B1 and D at T6: R344-38 (#309, W344), Sonnet

T6 had extended two guard tests of `reading_main` in its fix and run ci with names ending in a separator (`e2e `,
`git_programs `), which matched no test. The original then ran ci naming them, handed in, and was sent back at R344-41
for no result before the change.

| Run | Old texts | New texts |
| --- | --- | --- |
| 1 | ci naming the two and two more; no ci_before | handed in, the two saying in `not_before` they were not run with ci_before |
| 2 | handed in with no run of ci naming the two | ci and ci_before, each naming the two, the test file put on the code before |
| 3 | handed in, the same | ci and ci_before, the same |

Under the old texts, two of three hand in with neither test reported on the head (B1) and none runs ci_before (D). Under
the new, two of three run both, as told; the third hands in, misusing `not_before`, and would be sent back for the head.

### C and B1 at T1: R329-22 (#293, W329) and R301-37 (#208, W301), Sonnet

R329-22 was sent back with T1's label `red` where the latest ci_before reported the test not built, and with tests no ci
on the head reported (the only ci on head ba294656 had named two). R301-37 was sent back with `cannot_run` where
ci_before reported a test failed.

| Run | Old texts | New texts |
| --- | --- | --- |
| R329-22, 1 | ci naming five of the table's tests | handed in; each label as the latest ci_before that reported the test gives it |
| R329-22, 2 | handed in; each label as the latest ci_before that reported the test gives it | ci_before again for three tests whose run's file changed |
| R301-37, 1 and 2 | handed in; each label as the latest ci_before that reported the test gives it | the same |

Given the earlier results in full, neither text repeats the original's wrong labels, so Ch3 shows no change here and no
harm. B1 at T1 is not removed by the tool's text alone: the two hand-ins of R329-22 (one old, one new) name tests no ci
on the head reported, and the old run's ci named only five. Without the earlier results (R301-37 first rebuilt), both
texts first queried the records.

### B1 at I1: R355-10 (#206, W355), Sonnet

The original fixed a red lint and test and ran ci naming the ten tests the fix touched, then handed in all twenty-six
and was sent back. As a new session, the old texts ran ci naming all of them with ci_before for the changed file; the
new texts queried the earlier runs' requests twice (given as they stood), then ran ci naming all twenty-six with
ci_before for the two changed files. A new session recalls nothing of the earlier run, so it names every test either
way.

## 5. Conclusion

The changed checks take every A1 return that was sent back for it alone (R308-81, R329-64, R368-28, R368-29) and six
problems of R329-22, with nothing else in those returns changed. At T6 the new texts turn two of three runs from handing
in untested to running both ci and ci_before on the extended tests, the case of D and of B1 there. T1's labels and I1's
naming did not repeat their original errors in a new session under either text, so the new texts show no change and no
harm there. B1 at T1 remains in one of two runs: the tool's text alone does not make T1 name the earlier tests in its
last ci; the result field the findings left optional would tell it by the tool itself.
