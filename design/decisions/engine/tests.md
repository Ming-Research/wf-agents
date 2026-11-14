# A project's tests

How the engine runs a project's tests and reads what became of them.

### decision tests-run-where-the-host-checks
An agent has its tests run by asking the engine for `run_tests`, which runs them where the host checks the work; where the host is GitHub, that is the project's workflows run by hand on the last commit of the work's branch, which the engine pushes, and where it is the local repository, the project's check command run on the worktree as it stands, uncommitted changes included. Nothing is run on the machine the agents run on.
Why: Run in an agent's own shell, a heavy command fights the owner for the machine, appears in none of the engine's records, and on GitHub would need the agent to push, which puts the engine's credential within its reach. Letting one exact command through an agent's command line was turned down for the same reasons.

### decision only-a-commit-is-checked-on-github
Where the host is GitHub, `run_tests` refuses a worktree with changes not committed and pushes nothing, while a local run is on the worktree as it stands.
Why: CI can judge only a pushed commit, and a verdict must belong to the commit it judged, or it would be taken for that of changes it never saw; locally the command costs nothing more to run on what the worktree holds, and an agent may want to try before it commits.

### decision the-engine-reads-results-only-from-junit
The engine relies on nothing of a project's tests but their results as JUnit XML, each test with its name, outcome, time and the output of what failed; a run that uploads no JUnit is answered as such.
Why: A project's logs have no form the engine can read for every project: the engine once read a red job's log for what failed, which tied it to one project's runners, and the guesses went wrong where a log changed.

### decision a-test-has-one-name-matched-exactly
A test has one name, the one its results give it, which is the string an agent asks for it by and the project's fast workflow takes to run it; the engine treats it as it is, and it names exactly the tests that have that name, never those it is part of.
Why: The engine once cut a name at separators, which tied it to one project's runners and went wrong where a runner wrote names otherwise. That a name part of others should not name a test rests on judgment: it would run, and report, tests nobody asked for, and a mistyped name would pass by running something else.

### decision an-unbuilt-unit-is-answered-as-not-built
A unit that was not built is reported in the results as a unit with no test and an error, and a name that no test of such a run has is answered as not built, never as no such test.
Why: A test's name does not say which unit it is in, so in a run where some unit was not built, a name no test has may belong to that unit, and the engine cannot tell it from a name no test has; answering that there is no such test would send the agent to rename a test that exists (principle nothing-passes-for-normal). The engine reads only the results, so that report is its one way to learn that a unit was not built.

### decision a-project-declares-a-fast-and-a-full-workflow
The project declares two workflows with fixed roles. The fast workflow builds and runs only the tests and checks it is given, and every request of `run_tests` that names any is run by it; the full workflows hold every job required to merge, and a request that names none is run by them, on the exact commit, once, with the time a run may take given. Each says where it runs, a runner the installation names for the project's repository, or the run is not made.
Why: The fast run decides no merge, since it ran only what was named and, on the owner's machine, builds on what earlier runs left, so that its green could be one a clean build would not give (judgment, no such case recorded); the merge is judged by the full workflows (decision the-check-before-the-merge-is-the-full-workflows-once). The whole CI took three to five minutes a run, while a fast run gave its tests' results in about half a minute; running the full workflows for every request was turned down for that wait.

### decision the-fast-workflow-is-started-only-by-hand
The project's fast workflow is started only by the engine's dispatch by hand, never by a pull request or a push.
Why: It runs on the owner's own machine and the repository is public: a pull request's trigger would run anyone's code there, while by hand only someone who may write to the repository can ask.

### decision the-workflows-are-run-by-mains-file
Each workflow is always asked for on main, run by main's file of it with the commit to test given, and its run is found by a value only that request knows, which the run's name holds.
Why: Asked on the work's branch, the branch's own file would choose what runs on the project's machine; a branch made before that file would need a commit with main's CI files pushed for it, which a token without leave to write workflows cannot push; and two requests of one work at once, each taking the newest run, would read each other's.

### decision a-check-is-not-tried-again
Tests that could not be run, because their CI never started, took too long or the host could not be reached, say why in the result of `run_tests` once and are not run again by the engine; the agent may ask again.
Why: Its CI does not start because it is waited for again, and three tries of such a wait once spent half an hour for nothing; the agent who asked can tell whether to ask again, within the calls its run is allowed.

A check that ran and failed is not run again either: see the host's decision the-check-before-the-merge-is-the-full-workflows-once.

### decision a-tests-time-is-kept-with-the-run
What a request of `run_tests` gives of each test includes the time its results give, and the result is kept whole in the records with the run that made it, so that a test's time has a history there.
Why: A test's time is the only means to see a test grow slow, and it is in the results of every run already; kept only in what was given to an agent, it would be lost as soon as the round ended.
