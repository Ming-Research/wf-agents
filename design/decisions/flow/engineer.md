# The engineer's flow

What the engineer's flow is from its start to the merge: its cells and their order, and what is true of all of them.

### decision the-cells-are-laid-out-from-what-each-solves
The flow's order is S1, R0, R1, R2, D1, D2, P1, P2, I1, T1, T2, T3, T4, M1, M2, and each agent cell's file opens with the problem it is there to solve.
- replaced: The old engineer's flow, of about twenty-five cells, was on main; the owner changed it, because in the new design the cells were to be laid out from the problem each one solves, since copying the old cells would bring their problems back.
- rejected: an agent proposed mapping the old cells onto thirteen new ones; the owner rejected it, because what he wants is the problems solved, by a layout that follows the new design, not the old cells copied.

### decision the-work-is-started-by-the-owner
S1 runs on `start`, which the engine writes when the owner first lets the work begin, and only once the items it depends on have merged.
- replaced: A rule that a cell runs once if it has never run was on main; the owner changed it, because that rule was an exception for two cells, and the work starts when the owner lets it begin.
- rejected: an agent proposed that S1 run on `dependencies`; the owner rejected it, because for now the work is started by hand, and if the start is made automatic later, it will write the same datum.

### decision weight-is-decided-in-the-work
How heavy a work runs is decided during the work: R1 says in `deep_needed` whether deep research is needed, and T2 in `needs_testing` whether adversarial testing is; the item carries no mark of it.
- replaced: A light or a full route, which the architect marked when it made the item, was on main; the owner changed it, because the route should follow how hard the task really turns out to be, judged as the work is done, and almost no item was marked, so all ran the heavy way.
- rejected: the owner proposed two separate light flows, one for a bug fix and one for a small settled feature; the owner rejected it, because a light route kept in the engineer's flow must be as good as it can be, and chosen by the task's real difficulty.
- rejected: the owner proposed that a simple fix made in the research go straight to the reviews; the owner rejected it, because a way out to the reviews would bring the implementation's checks into the research, and simple works lose their time elsewhere.

### decision every-line-is-given-the-engineering-rules
Every line of the flow is given the engineering rules as well as the general ones: the review and testing lines as much as the engineer's line.
- replaced: Giving only the general rules to the lines of the plan review, the code review and the adversarial testing was on main; the owner changed it, because a reviewer must not judge by less than the writer, and the testing writes tests, which the engineering rules govern.

### decision opinions-are-inputs
What a judging or doubting cell says about a cell's work goes into their dialogue, and that cell answers by running again; no separate cell fixes what was objected to or checks the fix.
- replaced: A cell that fixed what the review found, and another that checked the fix, was on main; the owner changed it, because a fix is an increment: the old chart handled it badly, and the new one runs it as the implementation running again on what changed.

### decision a-review-runs-only-on-what-it-judges
A review runs only on a new version of what it judges or of its dialogue: neither `background` nor a reopened session runs P1, P2, T1, T2 or T3.
- replaced: Running the reviews also on `background` and on a new session of their line was on main; the owner changed it, because a changed background reaches a review through the plan, which its writer changes, and a reopened session gives a reviewer nothing to review again.
- rejected: an agent proposed that a new session run the design review, so that a reopened session reads the tree first; the owner rejected it, because the design review reviews the plan, so only a new plan runs it.

### decision the-tree-runs-no-cell
The design tree is a datum that the cells reading it are given as the files changed since their session read it; no cell watches it, and a changed tree is read when the line goes on.
- replaced: A changed design tree running R0 again was on main; the owner changed it, because R0 needs only one of main and the tree, and the tree given as context reaches the next cell that enters the session.
- rejected: an agent proposed that the design tree review watch the tree, so that the works under way are reviewed again when the tree changes on main; the owner rejected it, because that is rare, and it is main moving on, which runs the check before the merge.

### decision main-is-watched-by-the-check-alone
Main runs only the check before the merge; R0, D1 and I1 take main in as their runs start and are told what came in, but they do not watch it.
- rejected: the owner proposed that main, like the tree, run no cell and reach every cell only as context; the owner rejected it, because before the merge the check must run again on the new main, which is a real run, not information to fill in.

### decision the-work-ends-at-the-merge
The work ends at the merge: no cell watches main's CI after it, or checks the change on real runs once it is in.
- replaced: A cell that watched main's CI after the merge, holding the work's slot, was on main; the owner changed it, because it never caught anything: CI was green all 34 times, and the cell held slots for 3.9 hours.
- replaced: Cells that checked a merged change on real runs was on main; the owner changed it, because what runs for real can run in the tests, and in a trial they checked four works and found nothing real.
- rejected: an agent proposed a cell after the merge that checks the acceptance on the deployed system, with a tool that reads the project's pages; the owner rejected it, because wf-agents has no live site.
