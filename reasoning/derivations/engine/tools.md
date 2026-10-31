Decision query-rows-objects: A query's rows reach the agent, and the round's record, as a list of objects, each with a field for each column, and rows too long to give inline are saved one JSON object per line and given by the path in a field of their own, instead of one text of JSON lines inside the result, because a text inside JSON escapes every quote of every row, which the agent then reads through; the flow's schema for the tool cell's return says so, a row being an object with no fields listed, which holds whatever fields its value has, since the engine checks its own tool cell's return against that schema.

Rests on: C:information, F:json-text-escapes

Decision query-error-columns: A query that names a column or a table there is not fails with its error followed by the columns of each table and view it names, and for a table there is not, the name of every table and view agents read, from the index `format:common` lists them in, instead of the error alone, because in the sixth trial agents guessed columns the index listed (`work_state.paused_by`, `work_state.project`, `rounds.round`, `architect_queue.id`, `item_blocked_by.kind`), which a message of 80,000 characters did not let them find, and each guess cost a round; told the columns where it went wrong, the next round names them right.

Rests on: C:information, C:efficient, F:sixth-trial-guessed-columns, F:told-columns-named-right

Decision log-tail-clean: A red check's log tail is given without what gh puts before every line (the job, the step and the time) and without the escapes that colour the text, which gh writes with the escape character as the two characters "^[", because those take most of each line and say nothing about why the check is red; the times of the named tests are read from the logs cleaned the same way.

Rests on: C:information, F:gh-log-noise

Decision log-tail-failure: A red check's log tail is what of its log tells why it is red, and the whole log stays at the check's `log_path` for an agent to read with its tools: the steps that failed (those with an `##[error]` line), each from its command, without the runner's set-up and clean-up, the steps that passed, the step's shell and environment, cargo's and nextest's lines that only show progress (a crate fetched or built, a test started, passed, skipped or slow) and repeated blank lines; given whole up to 80 lines, and past that as each test that did not pass with its output, the summary to the end of the step and each error, warning or rustfmt diff with the lines under it up to a blank line, what is left out between them said as how many lines; at most 120 lines, the first and the last 30. Instead of the log's last 60 lines, because in the sixth trial those were mostly the runner's clean-up and tests that passed, at least 97,000 of the 223,000 tokens of log tails agents were given, and often missed the start of the failure they were given for.

Rests on: C:information, C:efficient, F:sixth-trial-log-tails

Decision ci-test-times: The ci tool gives, for each test the agent named, that test's own time, read from the line cargo-nextest writes as each test ends in the logs of every job of the fast CI, the jobs that each run a part of the tests included, with how it ended when it did not pass, or that the test was not found there, because without a time the agent fetched the whole job log to find one, and the time of the whole test binary that holds it, all `cargo test` gives, says nothing about the one test, while nextest runs each test on its own and times it; a fast CI that runs its tests another way gives the agent no times.

Rests on: C:information, C:efficient, F:agent-fetched-job-log, F:cargo-test-binary-time, F:nextest-times-each

Decision tree-check-counts: `wf-agents tree-check <fork point>` is a design tree check a project can name as its `design_check`: it counts the design tree under a directory (`design` unless `--root` says another) at HEAD, its nodes (the Markdown files), their depth below the root and the decisions (the paragraphs that start "Decision:"), and how many more of each than at the fork point, because the design tree review is to be given those numbers, and without a check named nothing printed them.

Rests on: C:information

Decision tree-check-format: `wf-agents tree-check` also fails, naming each fault with its node and line, when a node added or changed since the fork point is out of the node format the rule of design tree writing (`design_tree_writing`) sets, and leaves the nodes not changed as they are, instead of only printing numbers, because that rule says the engine rejects a return whose node is out of the format, and the check a project names is how it does so; nodes written before the check are held to it from their next change, since one out of the format would otherwise stop every work that changes the tree.

Rests on: C:flow-does, C:efficient, D:tree-check-counts

Decision not-caused-check: A red check a fix says its change did not cause (`not_caused_by_change`) is checked by the engine: it must be in the project's table of known red checks or red on main as the worktree last fetched it, read from the project's full CI; when main's checks cannot be read, the answer stands as written; instead of taking it as written, because a red check waved off as not the change's own is how a change breaks main.

Rests on: C:design-on-merit, F:waved-off-red-breaks-main

Decision github-english: What the engine writes to GitHub of its own is in English and holds none of the owner's words or an item's text, which are in the owner's language: a pull request the engineer has handed in no title and description for, at its first push (the ci tool) or when the full CI opens it, is opened as "Item" and the item's number, with a description saying that its title and description are the engineer's to hand in, and the engineer's `pr` at a later push replaces both; a pull request closed because its work ended says only that its work ended without merging, and one closed for the sub-items that took its work over names them by their numbers; instead of the item's title, problem and why, the reason the item was cancelled and the sub-items' titles, because the owner ruled that everything in a pull request is in English, and an item is written in the owner's language.

Rests on: F:repository-english, D:item-language

Decision pr-base-main: A work's pull request merges into the project's main (`main` in its configuration) and nowhere else: whenever the engine opens or updates a pull request (the ci tool, the full CI on the draft), it sets the pull request's base to main when it is another branch, and before it merges one (cell `M2`, and the revert of a merge that broke main) it does so again, merging nothing and failing, with both branches named, when the base cannot be set; instead of merging into whatever base the pull request has, because a pull request keeps the base it was opened with, while a work taken over keeps the pull request of the work before it and a project's main may change: in a trial whose project's main was a sandbox branch, a work taken over held a pull request opened against the real development branch, and merging it would have put the trial's code there.

Rests on: C:design-on-merit, F:pr-keeps-base, F:trial-sandbox-base, D:takeover-start-cells

Decision ci-before-named-tests: The ci_before tool runs exactly the tests it is named, each to its end, where the project's fast CI takes the tests to run: the project's configuration names that input of its workflow (`[ci] tests_input`), and the engine pushes the code before the change to a temporary branch (`wf-agents-before/...`) that the workflow's runs on push leave alone, then runs the workflow on it by hand with the tests named, which skips the lint and runs those tests only, none stopping the others; instead of the whole fast CI the push ran, which stops a job at its first red test, because on the code before the change a new-behaviour test must fail, and in the sixth trial adversarial testing found three times that such a test was cancelled before it ran when an earlier one failed, so that nothing measured it, while the tests named did not narrow the run at all, though the tool's words said they did. How a project's CI picks tests is the project's own, so it is the project's workflow that takes them and its configuration that says so; a project whose CI takes no tests keeps the whole run, and the result says which ran (`run`: `named` or `whole`). The lint is skipped on such a run because it checks main's code with only the tests added.

Rests on: C:information, C:independent, F:fast-ci-stops-first-red, F:sixth-trial-cancelled-tests, F:before-run-lint-main

Decision ci-whole-run: The ci tool keeps the run its push starts, the whole fast CI, which stops a job at its first red test, because its job is the whole CI green, a red run sends the work to the fix anyway, and a fast CI whose red run stops early is part of what the owner made it for.

Rests on: C:efficient, F:fast-ci-stops-first-red

Decision named-test-status: Both tools give each test named what the run's logs show of it, read from the line cargo-nextest writes as a test ends: passed with its time, failed with how it ended, not run when no log shows it, as when its job stopped at an earlier red test or there is no such test, or unread when the logs could not be read; instead of a time for each, because a test the run did not reach said only that it was not found, which could be read as measured.

Rests on: C:information, F:nextest-times-each, D:ci-test-times

Decision not-run-value: A new test that ci_before reports not run is handed in as not run before the change (`not_run`), in the table of new-behaviour tests and in that of old-behaviour tests alike: the engine takes it as it takes a new-behaviour test that cannot run, refusing no round for it and sending no work to the fix, and the table review judges whether what the test should show before the change, a new-behaviour test failing or an old-behaviour test passing, is shown some other way, a finding when it is not; the engine checks the value against ci_before's report of the test, refusing one said red or green that it reports not run, which nothing measured, and one said not run that it reports passed or failed; instead of only red, green or cannot run, none of which is true of a test the run never reached, because where a project's CI takes no tests to run ci_before runs the whole fast CI, which stops a job at its first red test, and in the sixth trial a work had two such tests and could not hand in its table truthfully, so that it had to ask the owner. An old-behaviour test not reached is unmeasured, not shown to fail: refusing it, or sending it to the fix, would push the engineer to rearrange tests around the tool.

Rests on: C:information, F:fast-ci-stops-first-red, F:sixth-trial-unreached-tests, D:ci-before-named-tests, D:named-test-status
