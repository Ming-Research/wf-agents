### decision tests-run-where-the-host-checks
An agent has its tests run by asking the engine, which checks the work where the host checks it; where the host is GitHub, that is the CI of the last commit of the work's branch, which the engine pushes, or the project's fast CI for what the agent names, and nothing is run on the machine the agents run on.
Why: Run in an agent's own shell, a heavy command fights the owner for the machine, appears in none of the engine's records, and on GitHub would need the agent to push, which puts the engine's credential within its reach. Letting one exact command through an agent's command line was turned down for the same reasons.

### decision only-a-commit-is-checked-on-github
Where the host is GitHub, the check refuses a worktree with changes not committed and pushes nothing, while a local check runs on the worktree as it stands.
Why: CI can judge only a pushed commit, and a verdict must belong to the commit it judged, or it would be taken for that of changes it never saw; locally the command costs nothing more to run on what the worktree holds, and an agent may want to try before it commits.

### decision before-the-change-on-a-branch-of-its-own
The code before a change is checked as one commit made of where the change begins with the agent's test files put on it, the same commit each time it is made of the same files, pushed to a branch of its own that is taken away once its CI is read, and never to the work's branch or its pull request.
Why: An agent committing its test before its fix serves the implementing agent alone: an adversary writes its tests after the change. Reverting the change on the work's branch was turned down: it asks an agent for exact work a program does, and puts a red commit in the pull request. The project's CI must therefore run on those branches: its fast CI by hand, or its whole CI on their push.

### decision where-the-change-begins-from-main-as-last-fetched
The tools tell where a work's change begins from main as the engine last brought it in, and never fetch main themselves.
Why: A fetch outside the lock the acts hold while they work in the repository would race their own fetches and merges; main as last brought in is never older than what the work's branch took in, so where the change begins comes out the same.

### decision named-tests-run-by-the-fast-ci
Where the project has a fast CI, the tests and checks an agent names are run alone by it, on the owner's own machine, and a check that names none is the whole CI; the whole CI stays what the check before a merge waits for.
Why: The whole CI took three to five minutes a check on GitHub's machines, and an agent checks many times a work, while a fast run gave its tests' results in half a minute to a minute and a quarter; running the whole CI on every check was turned down for that wait. A fast run decides no merge, since it ran only what was named.

### decision the-fast-ci-runs-by-mains-file
The fast CI is always asked for on main, run by main's file of its workflow with the commit to test given, and its run is found by a value only that request knows, which the run's name holds.
Why: Asked on the work's branch, the branch's own file would choose what runs on the project's machine; a branch made before that file would need a commit with main's CI files pushed for it, which a token without leave to write workflows cannot push; and two checks of one work at once, each taking the newest run, would read each other's.

### decision before-the-change-without-a-fast-ci
Where the project has no fast CI, the check before the change waits for the whole CI on its branch rather than being refused.
Why: Refused, such a project's tests would have no evidence of red before the change and green after it, and the system would serve only projects with a fast CI (Article 1).

### decision a-check-is-not-tried-again
A check that could not be run, because its CI never started, took too long or the host could not be reached, says why in its result once and is not run again by the engine; the agent may ask again.
Why: Its CI does not start because it is waited for again, and three tries of such a wait once spent half an hour for nothing; the agent who asked can tell whether to ask again, within the calls its run is allowed.

A check that ran and failed is not run again either: see the host's decision a-red-check-holds-the-work.

### decision a-query-is-refused-writes-by-the-connection
The engine runs an agent's `query` on a connection to the store that refuses every write itself: opened read-only, with `query_only` on, and with an authorizer that allows only a select, the reading of what is not hidden from agents, and functions; nothing may be attached. A query is bounded in rows, in time, in bytes (a row may not take more than the bytes a result is kept with, the first included, and one over it is refused), and in the length of any value, however it is made. A query withdrawn is interrupted.
Why: A check of the SQL's text for writes was turned down: it lists dangerous words, and an agent writes one more past the list (principle agent-writing-is-data). The time bound runs between the VM's instructions, so one call of a function over very long values is not stopped by it; the length of a value is what bounds that call's cost, whatever builds the value, where naming the functions that build large values (`zeroblob`, `randomblob`, `printf`) is again a list to write past.

### decision a-query-result-is-kept-whole-once-and-given-cut
A result of `query` is kept whole in the records, once per call, and a message gives a long one cut to the first rows, with its size and the query that reads the rest, and, where the first row alone is too long, that row with its long values shortened, so that each cut moves on.
Why: What ran is kept whole (principle the-past-as-it-ran): a query run again later may answer otherwise, since rows are added, and the agent is told what it saw. Principle stored-content-is-kept-small is met in that the copy is of what the agent asked for, once, and bounded by the bytes a result is kept with; a reference to the query alone was turned down, as it would keep no more than the query. The agent's context holds only what it can use, which is why the message is cut and the records are not.

### decision the-stores-comments-are-what-agents-are-told
What `{store}` in the words of `query` is filled with, by `carry::store`, is made from the comments of `db/schema.sql`, each table's and each column's. A table or a column whose comment says "hidden from agents" is left out of it and cannot be read by a query. The tool is off until the settings open it (`query.open`), and cannot be opened while the schema hides nothing, which is a fault at start: what agents may see of the store, and under which names, is the owner's to mark. With it off, a request is answered from the tool's file that the store is not open.
Why: A description written apart from the schema drifts from it. Marking nothing hidden would give agents the machine's paths and what they are not meant to read by default; the owner is made to choose.
