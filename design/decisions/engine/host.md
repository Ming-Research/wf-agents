# The host

Where a project's repository is hosted, GitHub or the local repository, and what the engine does there to check and merge a work.

### decision one-interface-for-the-host
The engine reaches a project's host through one interface: on GitHub it pushes the work's branch, opens a pull request, runs CI and merges there; with a local host it does all of it in the clone.
- rejected: an agent proposed doing every work in the local repository alone, running its check command and merging the branch there; the owner rejected it, because the project's rules have every change go through a pull request and its tests run only in CI.

### decision the-engine-merges-the-pull-request
The engine merges a work's pull request itself once the check before the merge passes; the owner reviews no pull request.
- rejected: an agent proposed asking the owner to review a work's pull request before it merges; the owner rejected it, because he reviews no pull request, as the process already said, and his questions go where he works.

### decision the-repository-is-worked-in-one-act-at-a-time
The engine's acts and its watch on main work in a project's repository one at a time, under a lock held in the engine's process.
- rejected: an agent proposed a file lock in the git directory; the owner rejected it, because a lock in the process is reached by every caller, and the git commands it guards are fast, so a wait for it costs little.
- rejected: an agent proposed trying a git command again when git reports a lock; the owner rejected it, because retrying hides real failures behind expected ones.

### decision the-check-takes-main-in
The check merges main's head into the work's branch in a commit of the engine's own and judges that commit, and the merge lands only a commit that holds the version of main the check took in.
- rejected: an agent proposed GitHub's merge queue; the owner rejected it, because it cannot be used for every repository and knows only the required checks, and two ways would give two sets of rules.

### decision the-check-before-the-merge-is-the-full-workflows-once
The check before the merge runs the project's full workflows once, by hand, on the exact commit to merge, and judges it by their JUnit and their jobs' conclusions.
- replaced: Judging the merge by the checks GitHub ran on the pushes of the pull request was on main; the owner changed it, because the full workflows are dispatched on the exact commit, once, before the merge.

### decision a-failed-job-is-said-by-its-failed-step
Where a job failed with no failed test of its own to say why, the engine reads the job's log from GitHub and gives the end of the step that failed it.
- replaced: Saying such a job only as failed, with its link, was on main; the owner changed it, because an implementation told only that the lint failed had to ask the owner to paste the error from the log, or else bisect with more CI runs.
- rejected: an agent proposed that each project's workflows write why a job failed into its JUnit; the owner rejected it, because every project would have to change its protected workflows, and a job whose runner dies writes nothing.
- rejected: an agent proposed a tool for the agent to read a job's log itself; the owner rejected it, because the agents' tools are only the query and the tests (decision `the-tools-are-query-and-run-tests`).

### decision a-red-the-change-did-not-make-is-not-sent-back
Where only tests failed in the full workflows, the check runs those tests again: one that passes is observed as maybe flaky, one red on main too makes the work wait for main, any other sends it back.
- replaced: Any red in the check holding the work until the owner let it go was on main; the owner changed it, because a red the change did not cause, such as a test that sometimes hangs, held every work it met for the owner.
- rejected: an agent proposed letting through every red the change did not cause, with an item to fix it; the owner rejected it, because while main is red, a change that broke the same test would not be seen.
- rejected: an agent proposed holding the work for every such red until an item fixes it; the owner rejected it, because one red would then hold every work after it.

### decision the-check-judges-the-commit-alone
The check before the merge judges only the work's commit: what the worktree holds besides it is no part of what is judged, and where the check cannot merge main for it, the work waits, saying why.
- replaced: The check failing while the worktree held anything not committed, untracked files too, was on main; the owner changed it, because that state is in no datum, so once the leftover was deleted nothing ran the check again and the work stopped before the merge.

### decision the-local-check-runs-on-a-checkout-of-the-commit
The local host runs the project's check command on a clean checkout of the commit to merge, made and taken away holding the repository, the command itself running without it.
- replaced: Running the command in the work's worktree as it stood was on main; the owner changed it, because the check no longer refuses what the worktree holds, so a file left out of every commit would pass and merge a commit that lacks it.
- rejected: an agent proposed running it in the worktree and refusing any untracked or uncommitted file there; the owner rejected it, because on a local host a leftover would still stop the work before the merge, and it is a second way beside the rule for every host.
- rejected: an agent proposed making and taking away the checkout without the repository lock, with an exception to decision the-repository-is-worked-in-one-act-at-a-time; the owner rejected it, because the exception rests on a marker git writes, which no test shows.
- rejected: an agent proposed a separate checkout made by `git clone --shared` outside the repository; the owner rejected it, because a clone carries none of the repository's filters, such as LFS, so the check would see pointer files, not the commit that merges.

### decision the-pull-request-says-what-the-work-wrote
A work's pull request is a draft until just before its merge, and its body is what the work wrote in the blocks the check names, with nothing in it left that GitHub would act on.
- replaced: One fixed paragraph as the body of every pull request was on main; the owner changed it, because GitHub then showed nothing of what changed.

### decision a-continued-open-pull-request-is-closed
Once a continuing work's own pull request is open, the engine closes the open pull request into main of what the item continued, with a comment naming the new one; a work's own is never closed.
- rejected: an agent proposed leaving the old pull request open for the owner, the replacement written only in the work's text; the owner rejected it, because two pull requests would stay open for one change, and GitHub would not show which replaced which.

### decision a-merge-is-named-by-its-works-line
A work's merge, its pull request and the commit it makes, is named by the first line of the block the flow's merge names, which the cell that writes the code writes in English.
- replaced: The item's title naming the pull request and the merge's commit was on main; the owner changed it, because the repository is written only in English, and items are in the owner's language.

### decision a-queued-run-is-cancelled-only-where-no-runner-is-online
A run no job of which has started that waits for a runner past `github.start_minutes` is cancelled only where none of its label is online, on the repository or its organisation; else it waits.
- replaced: Cancelling every run no runner took within `github.start_minutes` was on main; the owner changed it, because with every runner busy the runs of later requests were cancelled behind a long queue and the works stopped to ask again.
- replaced: Cancelling the same way a started run whose jobs wait for a runner where none is online was on main; the owner changed it, because a run whose jobs had run lost their work.

### decision a-run-is-under-way-once-a-job-of-it-has-started
A run is under way once a job of it has started, as its jobs say where GitHub says the run is queued, and it is never cancelled as never started.
- replaced: Judging whether a run started by GitHub's status of the run alone was on main; the owner changed it, because GitHub says a run queued again once a job has run and the next waits for a runner, and such runs were cancelled with their tests done.

### decision a-run-is-timed-only-while-it-is-not-waiting-for-a-runner
A workflow's `too_long` counts the time its run is not waiting for a runner, as the run's jobs say, and the time it waits while none of its label is online.
- replaced: Counting a run's time from the moment it was asked for, the queue included, was on main; the owner changed it, because a run whose jobs queued behind a busy machine was cut as too long for the time it waited, not the time it ran.
