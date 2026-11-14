### decision tests-run-where-the-host-checks
An agent has its tests run by asking the engine for `run_tests`, which runs them where the host checks the work; where the host is GitHub, that is the project's workflows run by hand on the last commit of the work's branch, which the engine pushes, and where it is the local repository, the project's check command run on the worktree as it stands, uncommitted changes included. Nothing is run on the machine the agents run on.
Why: Run in an agent's own shell, a heavy command fights the owner for the machine, appears in none of the engine's records, and on GitHub would need the agent to push, which puts the engine's credential within its reach. Letting one exact command through an agent's command line was turned down for the same reasons.

### decision only-a-commit-is-checked-on-github
Where the host is GitHub, `run_tests` refuses a worktree with changes not committed and pushes nothing, while a local run is on the worktree as it stands.
Why: CI can judge only a pushed commit, and a verdict must belong to the commit it judged, or it would be taken for that of changes it never saw; locally the command costs nothing more to run on what the worktree holds, and an agent may want to try before it commits.

### decision tools-use-main-as-last-fetched
The tools take main as the engine last brought it in, and never fetch main themselves.
Why: A fetch outside the lock the acts hold while they work in the repository would race their own fetches and merges; main as last brought in is never older than what the work's branch took in, so what the tools find of the branch against main comes out the same.

### decision every-project-meets-one-contract-for-its-tests
The engine relies on nothing of a project's tests but this: its results come as JUnit XML, each test with its name, outcome, time and the output of what failed, and the engine never reads a log to guess a result, so a run that uploads no JUnit is answered as such; a test has one name, the one its results give it, which is the string an agent asks for it by and the project's fast workflow takes to run only it, and the engine treats it as it is; and the project declares in `flow/project.toml` two workflows with fixed roles. The fast workflow builds and runs only the tests and checks it is given, and every request of `run_tests` that names any is run by it. The full workflows hold every job required to merge, and a request that names none is run by them, on the exact commit, once, with the time a run may take given; each says where it runs, a runner the installation has or the run is not made. The installation holds only what is of the machine: its token and the runners it has.
Why: A project's logs and the names its test runners write have no form the engine can read for every project: the engine once cut a name at separators and read a red job's log for what failed, which tied it to one project's runners, and the guesses went wrong where a log changed. One format and one name, met by the project's workflows, leave nothing to guess. The fast run decides no merge, since it ran only what was named, and the whole CI took three to five minutes a run, while a fast run gave its tests' results in a fraction of that; running the full workflows for every request was turned down for that wait. What the project is, its workflows and runners, is the project's and travels with its files, not the installation's.

### decision the-workflows-are-run-by-mains-file
Each workflow is always asked for on main, run by main's file of it with the commit to test given, and its run is found by a value only that request knows, which the run's name holds.
Why: Asked on the work's branch, the branch's own file would choose what runs on the project's machine; a branch made before that file would need a commit with main's CI files pushed for it, which a token without leave to write workflows cannot push; and two requests of one work at once, each taking the newest run, would read each other's.

### decision a-check-is-not-tried-again
Tests that could not be run, because their CI never started, took too long or the host could not be reached, say why in the result of `run_tests` once and are not run again by the engine; the agent may ask again.
Why: Its CI does not start because it is waited for again, and three tries of such a wait once spent half an hour for nothing; the agent who asked can tell whether to ask again, within the calls its run is allowed.

A check that ran and failed is not run again either: see the host's decision the-check-before-the-merge-is-the-full-workflows-once.

### decision a-query-is-refused-writes-by-the-connection
The engine runs an agent's `query` on a connection to the store that refuses every write itself: opened read-only, with `query_only` on, and with an authorizer that allows only a select, the reading of what is not hidden from agents, and functions; nothing may be attached. A query is bounded in rows, in time, in bytes (a row may not take more than the bytes a result is kept with, the first included, and one over it is refused), and in the length of any value, however it is made. A query withdrawn is interrupted.
Why: A check of the SQL's text for writes was turned down: it lists dangerous words, and an agent writes one more past the list (principle agent-writing-is-data). The time bound runs between the VM's instructions, so one call of a function over very long values is not stopped by it; the length of a value is what bounds that call's cost, whatever builds the value, where naming the functions that build large values (`zeroblob`, `randomblob`, `printf`) is again a list to write past.

### decision a-query-result-is-kept-whole-once-and-given-cut
A result of `query` is kept whole in the records, once per call, and a message gives a long one cut to the first rows, with its size and the query that reads the rest, and, where the first row alone is too long, that row with its long values shortened, so that each cut moves on.
Why: What ran is kept whole (principle the-past-as-it-ran): a query run again later may answer otherwise, since rows are added, and the agent is told what it saw. Principle stored-content-is-kept-small is met in that the copy is of what the agent asked for, once, and bounded by the bytes a result is kept with; a reference to the query alone was turned down, as it would keep no more than the query. The agent's context holds only what it can use, which is why the message is cut and the records are not.

### decision the-stores-comments-are-what-agents-are-told
What `{store}` in the words of `query` is filled with, by `carry::store`, is made from the comments of `db/schema.sql`, each table's and each column's. A table or a column whose comment says "hidden from agents" is left out of it and cannot be read by a query. The tool is off until the settings open it (`query.open`), and cannot be opened while the schema hides nothing, which is a fault at start: what agents may see of the store, and under which names, is the owner's to mark. With it off, a request is answered from the tool's file that the store is not open.
Why: A description written apart from the schema drifts from it. Marking nothing hidden would give agents the machine's paths and what they are not meant to read by default; the owner is made to choose.

### decision a-tools-result-is-kept-whole-and-given-cut
What a tool says is kept whole in the work's records, and cut only where a round is given it, at the bound its tool's words name.
Why: Cut where the host made it, the engine's records kept only what the agent was given, and what was cut could never be read again; the bound is the round's, not the result's.

### decision a-tests-time-is-kept-with-the-run
What a request of `run_tests` gives of each test includes the time its results give, and the result is kept whole in the records with the run that made it, so that a test's time has a history there.
Why: A test's time is the only means to see a test grow slow, and it is in the results of every run already; kept only in what was given to an agent, it would be lost as soon as the round ended.
