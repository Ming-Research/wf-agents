# Catching up with main

How a work's branch takes in what merged into main while the work goes on.

### decision a-run-that-changes-the-code-takes-main-in
Before the first round of each run of a cell marked `takes_main`, the engine merges main's head, as the host last gave it, into the work's branch; later rounds of the run take nothing in.
- replaced: A work held to the main it started from until the check before its merge stood on main; the owner changed it, because a work kept close to main to the last moment merges more often and is reworked less.
- rejected: an agent proposed pinning the main a work starts from, or recording only the commit it started on; the owner rejected it, because a conflict found only at the merge costs the most.
- replaced: Only the cells that change the code taking main in stood on main; the owner changed it, because the first reading of the design tree must read it as main has it, and a work that never took main in ended in a conflict.
- rejected: an agent proposed taking main in only at that first reading and before the merge, so that the implementation does not merge round after round; the owner rejected it, because it is not that frequent, and a merge left for later must still be made and reviewed.

### decision a-conflict-is-the-agents-to-merge
Where main does not merge without a conflict, or the worktree holds uncommitted changes or a merge, the engine takes nothing in and tells the round why in one line, with the files and no order.
- replaced: Listing every file main changed, and on a conflict telling the agent to merge it and then change the code, stood on main; the owner changed it, because the engine's words are then alike for every cell, the first reading of the tree among them, which is not to merge.

### decision what-a-run-took-in-from-main-it-has
What the round that takes main in reads of the branch after the merge, a new design tree among it, is put into that round, and runs its cell no more.
- replaced: A version brought in by a run's own merge of main taken as a change that ran its cell again stood on main; the owner changed it, because main is merged first and what changed judged after, both handled in the one run, where the research read the same files twice.
