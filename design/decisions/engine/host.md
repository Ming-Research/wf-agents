### decision one-interface-for-the-host
The acts and the agents' checks reach where the project's repository is hosted through one interface, implemented for the local repository and for GitHub. Everything else about a work's branch and worktree is done in the local clone.
Why: An act means one thing whatever carries it out, and the local host lets every act be tested with no network (principle outside-behind-interfaces). A test of the host in each act would spread it through the code, and a host per act lost because the check and the merge must agree on one pull request.

### decision github-through-its-command-line
GitHub is asked through its own command line, by its REST API and by GraphQL only for what REST cannot do. Git pushes and fetches go to the address the settings give, never one read from the clone.
Why: The command line handles authentication and paging and adds no network stack to the build (principle tech-mainstream). An agent can write the clone's configuration, so an address or proxy taken from there could send the token elsewhere.

### decision found-again-on-the-host
Nothing of a check on the host is kept by the engine: the pull request is found again by its branch, the CI by its commit. A pull request a person closed, merged or pointed elsewhere fails the round, and no second one is opened.
Why: A restart between opening a pull request and recording it would open a second one (principle effects-known-as-done), and opening another past a person's close would act against them.

### decision the-check-before-the-merge-is-the-full-workflows-once
The check before the merge runs the project's full workflows on the exact commit to merge, by hand and once, and judges it by the JUnit they upload and by their jobs' conclusions: it fails on a test that failed, on a job that did not pass, and on a run that uploaded no JUnit the engine can read, and never by the pull request's checks or a log. Each failure is said as it is, and no failed job is ever run again by the engine.
Why: What the pull request's checks show is what GitHub's triggers happened to start, and counting every check that appears cannot tell "all done" from "not all started"; the branch rules' required checks lost because reading them needs administration rights. The project's declaration names what must pass, and a log has no form the engine can read for every project. Running a red job again lost because it destroys the scene of an intermittent failure and lets it merge, when the task is to find it.

### decision the-check-takes-main-in
The check merges main's head into the work's branch as a commit of the engine's own and judges that commit, and the merge lands only a commit that holds the main the check took in.
Why: What passes is then exactly what lands. GitHub's test merge lost because it leaves no mark when main moves after the CI began and the local host has none; an agent's round lost as exact work an agent sometimes gets wrong (principle mechanical-engine).

### decision the-check-holds-the-head-to-what-the-work-names
The check passes only a branch whose head is a commit the work names in what its cells write (but the check's own), or such a commit with only the engine's own merges of main on top of it, each known by its address, its words and the main it merged; where the work names none, or none that is the head, the check fails with why. What only people write, and what a source outside writes, as main's head, names nothing.
Why: An agent that committed after handing in what it did, or named a commit it did not make, would have code merged that nobody judged by its words. The commit is read where the work already names it (the code, the tests a tester committed, an answer to a review or to the check, "fixed, with its commit") rather than from a field of its own: a judgment, no case having tried the other. A merge by the engine's address alone is no merge of the engine's: an agent can commit by any address.

### decision one-work-checks-and-merges-at-a-time
The works of a project take turns to check and merge: a work's check waits until no other work has the turn, and the work keeps it from its check until its merge, or its check failing; a work whose check passed and whose merge does not come keeps it no longer than a quarter of an hour. The turn is kept in memory only.
Why: Checked side by side, each work's merge moves main under the others', each of which is then checked again, and the last of many could go round past its bound on tries and stop for the owner. That a work held between its check and its merge holds the others back no longer than a quarter of an hour rests on judgment.

### decision a-moved-main-is-checked-again-by-itself
A merge refused because main moved since the check passed, as the engine finds or GitHub says ("Base branch was modified"), fails in passing and keeps the work's turn: the watch of main writes main's new head into the work at once, which runs the check again on it, and the merge is tried again after its wait with what that check found. Nothing waits for the owner.
Why: Main moves under a checked work where it was merged by hand or by another engine; a refusal that waited for the owner stopped the work for what passes by itself. Merging main in within the merge lost, since what lands would be what no check judged (decision the-check-takes-main-in).

### decision the-pull-request-says-what-the-work-wrote
A work's pull request is a draft until just before its merge; its body is what the work wrote in the blocks the flow's check names, published with no mention, reference, closing word or path of this machine left to act, cut to a bound, and written over at each check, a person's edit included; the check before the merge and the agents' check work it out alike.
Why: Anyone on GitHub sees what the change is and that it is not yet to be reviewed, and an agent's text on a public repository must notify, close and reveal nothing. Worked out otherwise by either check, the two would rewrite each other's body each time.

### decision letting-go-at-the-end
Once a work has ended and nothing of it is under way, the engine takes away its worktrees, uncommitted changes included on purpose, and closes its pull request into main where it did not merge; the work's own branch stays, and nothing that is a symbolic link is followed. What cannot be let go is the engine's pause of the ended work, saying why, tried again once the owner lets it on.
Why: A round or a tool may work in the worktree until it ends, the branch keeps the commits the work paid for (principle keep-what-was-paid-for), and a link could lead to another work's worktree. A failure retried unseen would hold a locked worktree for ever with nobody told (principle nothing-passes-for-normal).
