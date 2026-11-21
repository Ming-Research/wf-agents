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

### decision one-work-checks-and-merges-at-a-time
The works of a project take turns to check and merge, in the order their checks began to wait; a work keeps the turn from its check until it merges or its check fails.
- rejected: an agent proposed bringing each branch that is behind main up to date with it and checking it again, with the works side by side; the owner rejected it, because one pull request was brought up to date with main four times in an hour while main broke twice.

### decision the-check-before-the-merge-is-the-full-workflows-once
The check before the merge runs the project's full workflows once, by hand, on the exact commit to merge, and judges it by their JUnit and their jobs' conclusions.
- replaced: Judging the merge by the checks GitHub ran on the pushes of the pull request was on main; the owner changed it, because the full workflows are dispatched on the exact commit, once, before the merge.

### decision a-red-the-change-did-not-make-is-not-sent-back
Where only tests failed in the full workflows, the check runs those tests again: one that passes is observed as maybe flaky, one red on main too makes the work wait for main, any other sends it back.
- replaced: Any red in the check holding the work until the owner let it go was on main; the owner changed it, because a red the change did not cause, such as a test that sometimes hangs, held every work it met for the owner.
- rejected: an agent proposed letting through every red the change did not cause, with an item to fix it; the owner rejected it, because while main is red, a change that broke the same test would not be seen.
- rejected: an agent proposed holding the work for every such red until an item fixes it; the owner rejected it, because one red would then hold every work after it.

### decision the-pull-request-says-what-the-work-wrote
A work's pull request is a draft until just before its merge, and its body is what the work wrote in the blocks the check names, with nothing in it left that GitHub would act on.
- replaced: One fixed paragraph as the body of every pull request was on main; the owner changed it, because GitHub then showed nothing of what changed.

### decision a-merge-is-named-by-its-works-line
A work's merge, its pull request and the commit it makes, is named by the first line of the block the flow's merge names, which the cell that writes the code writes in English.
- replaced: The item's title naming the pull request and the merge's commit was on main; the owner changed it, because the repository is written only in English, and items are in the owner's language.
