Fact agent-fetched-job-log: Without a test's own time an agent fetched the whole job log to find one.

Fact agent-writes-null: An agent writes null for a field it has nothing for.

Fact attempt-rewrites-return: Each attempt at a return writes the whole return again.

Fact before-run-lint-main: A lint on the code before the change checks main's code with only the tests added.

Fact cancel-cuts-round: Cancelling an item, from the owner's page or by an architect's change, ends its work at once in the same transaction, cutting off the round the work is in, whatever its step is doing.

Fact cargo-test-binary-time: `cargo test` gives only the time of the whole test binary that holds a test.

Fact fast-ci-stops-first-red: The fast CI stops a job at its first red test.

Fact finish-round-unchecked: Ending a round changed no row when the round was no longer open and still reported success, so the step that ended it went on to move its work as if it had ended the round.

Fact first-message-varies: A session's first message begins with what differs from session to session: the git status, the working directory, the date.

Fact gh-log-noise: What gh puts before every line of a check's log (the job, the step and the time) and the escapes that colour the text, which gh writes with the escape character as the two characters "^[", take most of each line and say nothing about why the check is red.

Fact json-text-escapes: A text inside JSON escapes every quote it holds, which an agent reading it then reads through.

Fact main-merge-sent-back: Counted as a work's own, the changes a merge of main brought into its branch sent a return back for a design card or a change the work never made.

Fact most-flow-changes-keep-cell: Most changes to the flow leave the cell a work stands at and its returns as they were.

Fact nextest-times-each: cargo-nextest runs each test on its own and times it, writing a line as each test ends.

Fact no-line-reader-repeated: A reader on no line, sent back for its form in a new session told nothing of why, hands in the same form again.

Fact one-machine-enough: One machine is enough to run the flow first.

Assumed: the work outgrowing one machine overturns it

Fact ping-is-a-call: A ping that keeps a session warm is one call of the agent's command line, which runs until the command line returns.

Fact pr-keeps-base: A pull request keeps the base it was opened with.

Fact refusal-cost: A refusal within the run costs about what a round sent back costs the model: tried with Haiku, 4,763 tokens read, 212 written and 255 put out, against 4,737, 316 and 275 for the same return sent back in a resumed session.

Fact required-field-added: Haiku, told to hand in a given input that lacked a required field, added it unasked.

Fact session-holds-cut-off-round: A session holds everything a round cut off in it was given.

Fact startup-takeover-lost-rulings: Taking over every work on an older flow version as the engine started ended works whose design cards and asks the owner was about to rule on and answer.

Fact step-waits-minutes: A step of a work waits on an agent's command line or on a CI run for minutes: in the owner's database an agent's round took up to about seventeen minutes and a CI run up to about thirteen.

Fact steps-never-overlapped: In the owner's database no two works' agent rounds and no two works' CI runs ever overlapped in time, while several works were alive together, their slots taken.

Fact texts-filled-db: The shared formats, the rules, a cell's task and the schema are the same in round after round and session after session, and stored again in each round they made most of the database.

Fact told-columns-named-right: An agent told the columns where its query went wrong names them right in its next round.

Fact wait-set-over: A failed run's cause that stops every work set its wait over whatever wait stood, and the look at the network ended whatever wait stood once it was done, so that a later cause's shorter wait, or one a step set during the look, replaced or ended a longer one.

Fact waved-off-red-breaks-main: A red check waved off as not the change's own is how a change breaks main.

Fact whole-streaming-checked: The service checks the JSON of a tool input streamed whole.

Fact work9-session-tokens: Work 9's engineer session reached 312,000 tokens before implementing and 531,000 at its end, every call reading all of it again.

Fact wrong-change-reason-resent: An agent told it made a change it did not make takes the reason for a mistake and is sent back again.
