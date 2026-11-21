# The engineer's flow

What the engineer's flow is from its start to the merge: its cells and their order, and what holds across all of them.

### decision the-cells-are-laid-out-from-what-each-solves
The flow walks S1, R0, R1, R2, D1, D2, P1, P2, I1, T1, T2, T3, T4, M1, M2, and each agent cell's file opens with the problem it is there to solve.
- replaced: The old engineer's flow of some twenty-five cells stood on main; the owner changed it, because the cells were to be laid out from the problem each solves in the new design, as copying the old ones would bring their problems back.
- rejected: an agent proposed mapping the old cells onto thirteen new ones; the owner rejected it, because what is wanted is that the problems are solved, by a layout that follows the new design, not the old cells copied.

### decision the-work-is-started-by-the-owner
S1 runs on `start`, which the engine writes as the owner first lets the work on, and only once the items it depends on have merged.
- replaced: A rule that a cell never run runs once stood on main; the owner changed it, because that rule was an exception for two cells, and the work starts as the owner lets it on.
- rejected: an agent proposed that S1 run on `dependencies`; the owner rejected it, because the work is started by hand for now, and a start made automatic later writes the same datum.

### decision weight-is-decided-in-the-work
How heavy a work runs is decided while it is done: R1 says in `deep_needed` whether deep research is needed, T2 in `needs_testing` whether adversarial testing is; the item carries no mark of it.
- replaced: A light or a full route, marked by the architect as it made the item, stood on main; the owner changed it, because the route is to follow how hard the task really turns out, judged as it is done, and almost no item was marked, so all ran the heavy way.
- rejected: the owner proposed two light flows of their own, one for a bug fix and one for a small settled feature; the owner rejected it, because a light route kept in the engineer's flow must be as good as it can be, chosen by the task's real difficulty.
- rejected: the owner proposed that a simple fix made in the research go straight to the reviews; the owner rejected it, because an exit to the reviews would bring the implementation's checks into the research, and simple works lose their time elsewhere.

### decision every-line-is-given-the-engineering-rules
Every line of the flow is given the engineering rules beside the general ones, the lines of the reviews and of the testing as much as the engineer's.
- replaced: The general rules alone for the lines of the plan review, the code review and the adversarial testing stood on main; the owner changed it, because a reviewer must not judge by less than the writer, and the testing writes tests the engineering rules govern.

### decision opinions-are-inputs
What a judging or doubting cell says of a cell's work is written in their dialogue, which the cell answers by running again; no cell of its own fixes what was objected to or checks the fix.
- replaced: A cell that fixed what the review found and one that checked the fix stood on main; the owner changed it, because a fix is an increment, which the old chart handled badly and the new one runs as the implementation running again on what changed.

### decision a-review-runs-only-on-what-it-judges
A review runs on a new version of what it judges and of its dialogue alone: neither `background` nor a reopened session runs P1, P2, T1, T2 or T3.
- replaced: The reviews run also by `background` and by a new session of their line stood on main; the owner changed it, because a changed background reaches a review through the plan its writer changes, and a reopened session gives a reviewer nothing to review again.
- rejected: an agent proposed that a new session run the design review, so that a reopened session reads the tree first; the owner rejected it, because the design review reviews the plan, so only a new plan runs it.

### decision the-tree-runs-no-cell
The design tree is a datum the cells that read it are given as the files changed since their session read it, and no cell watches it: a changed tree is read where the line goes on.
- replaced: A changed design tree running R0 again stood on main; the owner changed it, because R0 needs only one of main and the tree, and the tree given as context reaches the next cell that enters the session.
- rejected: an agent proposed that the design tree review watch the tree, so that a tree changed on main has the works under way reviewed again; the owner rejected it, because that is rare and is main moving on, which runs the check before the merge.

### decision main-is-watched-by-the-check-alone
Main runs only the check before the merge; R0, D1 and I1 take main in as their runs start, and are told what came in, without watching it.
- rejected: the owner proposed that main, like the tree, run no cell and reach every cell only as context; the owner rejected it, because before the merge the check must run again on the new main, a real run and not information to fill in.

### decision the-work-ends-at-the-merge
The work ends at the merge: no cell watches main's CI after it, or checks the change on real runs once it is in.
- replaced: A cell that watched main's CI after the merge, holding the work's slot, stood on main; the owner changed it, because it never caught anything: green all 34 times, holding slots for 3.9 hours.
- replaced: Cells that checked a merged change on real runs stood on main; the owner changed it, because what runs for real can run in the tests, and in a trial they checked four works and found nothing real.
- rejected: an agent proposed a cell after the merge checking the acceptance on the deployed system, with a tool reading the project's pages; the owner rejected it, because wf-agents has no live site.
