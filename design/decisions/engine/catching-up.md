# Catching up with main

How a work's branch takes in what merged into main while the work goes on.

### decision a-run-that-changes-the-code-takes-main-in
Before the first round of each run of a cell marked `takes_main`, the engine merges main's head, as the host last gave it, into the work's branch; later rounds of the run take nothing in.
- replaced: Keeping a work on the main it started from until the check before its merge was on main; the owner changed it, because a work kept close to main up to the last moment merges more often and is reworked less.
- rejected: an agent proposed pinning the main a work starts from, or recording only the commit it started on; the owner rejected it, because a conflict found only at the merge costs the most.
- replaced: Having only the cells that change the code take main in was on main; the owner changed it, because the first reading of the design tree must read it as main has it, and a work that never took main in ended in a conflict.
- rejected: an agent proposed taking main in only at that first reading and before the merge, so that the implementation does not merge round after round; the owner rejected it, because it is not that frequent, and a merge left for later must still be made and reviewed.

### decision a-conflict-is-the-agents-to-merge
When main does not merge without a conflict, or the worktree holds uncommitted changes or a merge, the engine takes nothing in and tells the round why in one line, with the files and no order.
- replaced: Listing every file main changed, and on a conflict telling the agent to merge it and then change the code, was on main; the owner changed it, because the engine's words are then the same for every cell, including the first reading of the tree, which is not to merge.

### decision what-a-run-took-in-from-main-it-has
What the round that takes main in reads from the branch after the merge, including a new design tree, is put into that round, and does not run its cell again.
- replaced: Taking a version brought in by a run's own merge of main as a change that ran its cell again was on main; the owner changed it, because main is merged first and what changed is judged after, both in the one run, whereas running it again had the research read the same files twice.
