### decision one-interface-for-the-host
The acts and the agents' checks reach where the project's repository is hosted through one interface, implemented for the local repository and for GitHub. Everything else about a work's branch and worktree is done in the local clone.
Why: An act means one thing whatever carries it out, and the local host lets every act be tested with no network (principle outside-behind-interfaces). A test of the host in each act would spread it through the code, and a host per act lost because the check and the merge must agree on one pull request.

### decision github-through-its-command-line
GitHub is asked through its own command line, by its REST API and by GraphQL only for what REST cannot do. Git pushes and fetches go to the address the settings give, never one read from the clone.
Why: The command line handles authentication and paging and adds no network stack to the build (principle tech-mainstream). An agent can write the clone's configuration, so an address or proxy taken from there could send the token elsewhere.

### decision found-again-on-the-host
Nothing of a check on the host is kept by the engine: the pull request is found again by its branch, the CI by its commit. A pull request a person closed, merged or pointed elsewhere fails the round, and no second one is opened.
Why: A restart between opening a pull request and recording it would open a second one (principle effects-known-as-done), and opening another past a person's close would act against them.

### decision the-checks-the-installation-names
Only the checks the installation names count, each by its latest run, and one that ended without judging the code, such as a cancelled one, fails the round instead of the work.
Why: Counting every check that appears cannot tell "all done" from "not all started" and makes an informational bot a judge; the branch rules' required checks lost because reading them needs administration rights.

### decision red-beside-the-change
A named check that fails on the commit and on main's head as well holds no work, and one that failed in a run the work already read is run again once, a pass then said to fail now and then. This is apart from a check that could not run at all, which is not tried again (decision a-check-is-not-tried-again).
Why: Reading the same red run again tells nothing new, and a flaky test once sent a work round to someone who could fix nothing. Running every red again lost, as it gets past a real failure by luck; a table of known red checks lost, as someone must keep it while main's own CI says the same mechanically.

### decision no-merge-onto-a-red-main
No work merges onto a main whose named checks failed on its head, even one whose own check passed over that failure: the merge fails its round, pausing the work with the reason.
Why: Merging onto a red main hides which change broke it. Waiting inside the merge gains nothing, since a green main has a new head that the check must take in anyway.

### decision the-check-takes-main-in
The check merges main's head into the work's branch as a commit of the engine's own and judges that commit, and the merge lands only a commit that holds the main the check took in.
Why: What passes is then exactly what lands. GitHub's test merge lost because it leaves no mark when main moves after the CI began and the local host has none; an agent's round lost as exact work an agent sometimes gets wrong (principle mechanical-engine).

### decision the-pull-request-says-what-the-work-wrote
A work's pull request is a draft until just before its merge, and its body is what the work wrote in the blocks the flow's check names; the check before the merge and the agents' check both bring it up to date, working it out alike.
Why: Anyone on GitHub sees what the change is and that it is not yet to be reviewed; opened ready, unfinished work would show as ready. Worked out otherwise by either, the two would rewrite each other's body at every check.

### decision letting-go-at-the-end
Once a work has ended and nothing of it is under way, the engine takes away its worktrees and the host's branches of its checks before the change, and closes its pull request where it did not merge; the work's own branch stays. A failure is tried again with growing waits until it is done.
Why: A round or a tool may work in the worktree until it ends, and the branch keeps the commits the work paid for (principle keep-what-was-paid-for). A work that has ended can no longer be paused for the owner, as a failed round's is.
