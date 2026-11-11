### decision tests-run-where-the-host-checks
An agent has its tests run by asking the engine, which checks the work where the host checks it; where the host is GitHub, that is the CI of the last commit of the work's branch, which the engine pushes, and nothing is run on the machine the agents run on.
Why: Run in an agent's own shell, a heavy command fights the owner for the machine, appears in none of the engine's records, and on GitHub would need the agent to push, which puts the engine's credential within its reach. Letting one exact command through an agent's command line was turned down for the same reasons.

### decision only-a-commit-is-checked-on-github
Where the host is GitHub, the check refuses a worktree with changes not committed and pushes nothing, while a local check runs on the worktree as it stands.
Why: CI can judge only a pushed commit, and a verdict must belong to the commit it judged, or it would be taken for that of changes it never saw; locally the command costs nothing more to run on what the worktree holds, and an agent may want to try before it commits.

### decision before-the-change-on-a-branch-of-its-own
The code before a change is checked as one commit made of where the change begins with the agent's test files put on it, the same commit each time it is made of the same files, pushed to a branch of its own that is taken away once its CI is read, and never to the work's branch or its pull request.
Why: An agent committing its test before its fix serves the implementing agent alone: an adversary writes its tests after the change. Reverting the change on the work's branch was turned down: it asks an agent for exact work a program does, and puts a red commit in the pull request. The project's CI must therefore run on those branches.

### decision where-the-change-begins-from-main-as-last-fetched
The tools tell where a work's change begins from main as the engine last brought it in, and never fetch main themselves.
Why: A fetch outside the lock the acts hold while they work in the repository would race their own fetches and merges; main as last brought in is never older than what the work's branch took in, so where the change begins comes out the same.

### decision the-whole-ci-is-run
A check runs the project's whole CI; naming tests to run alone is not offered.
Why: It needs the project's CI to take the names as an input, and saves little while the whole CI takes between three and five minutes, as five of its runs took; naming tests pays where the whole CI takes far longer.

### decision a-check-is-not-tried-again
A check that could not be run, because its CI never started, took too long or the host could not be reached, says why in its result once and is not run again by the engine; the agent may ask again.
Why: Its CI does not start because it is waited for again, and three tries of such a wait once spent half an hour for nothing; the agent who asked can tell whether to ask again, within the calls its run is allowed.

A check that ran and failed is not run again either: see the host's decision a-red-check-holds-the-work.
