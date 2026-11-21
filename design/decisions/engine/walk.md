# The walk

What decides, from a work's records and the flow, which cell runs next, and what holds a work where it stands.

### decision the-walk-goes-from-the-first-cell
Each time something happens to a work, the walk goes through the flow's cells from the first, passing each that nothing new runs, and keeps no place where the work stands.
- replaced: Rows back drawn as routes of the state chart, with rules for sending a work back kept outside the chart, was on main; the owner changed it, because a refused approval is then only something made stale and a walk again from the start.
- rejected: an agent proposed a work as its products alone, with no cells and no place, its next step worked out from what is missing or stale; the owner rejected it, because cells are how he shapes the flow, as by a review added after the design, and products would carry the state machine anyway.
- rejected: an agent proposed that a cell run again makes every cell after it stale; the owner rejected it, because only a changed input a cell depends on makes it stale, and an output that did not change runs nothing after it.

### decision conditions-and-preconditions
A cell whose condition does not hold is passed, and what it wrote still counts until a newer version; a precondition holds the work at its cell, worked out at each walk and never stored.
- rejected: an agent proposed that what a cell wrote no longer counts while the walk passes it; the owner rejected it, because a cell not run leaves what it wrote valid, and only a veto, such as his approval refused, voids something.

### decision a-way-back-waits-for-the-pass
A cell run only by what cells after it wrote waits while a cell after it, short of the next with preconditions, is to run for something else, has a run at rest or waits for calls.
- replaced: Entering, at each walk, the first cell from the start that had something new was on main; the owner changed it, because each judge's point then went back to the plan's writer before the next judge saw the plan, three cells running four times each in one work.
- rejected: the owner proposed running as a batch every cell that had something new as the batch began, then the next batch from what they wrote; the owner rejected it, because the same comes from the records alone, with no batch recorded, and what a cell of the pass makes new runs in that pass.
- rejected: an agent proposed that the maker answers only once every review of what it made has seen its latest version; the owner rejected it, because the rules have only reads, writes and the runs reads bring, and a condition made for reviews would be a new mechanism.
- rejected: an agent proposed that a cell later in the pass is given the versions as they stood when the pass began; the owner rejected it, because two cells that change the code must not build on different code.

### decision meaning-told-by-the-writer
A choice or a list of names changes its meaning when its value does; prose changes it only where its writer says the change matters, and a version said not to matter is kept and runs no cell.
- rejected: an agent proposed taking every new text of a block as a change that runs the cells after it; the owner rejected it, because a mechanical comparison would run everything after a reworded text again.
- replaced: A version handed in as changing nothing in meaning written over the version it mends was on main; the owner changed it, because the cells that had the old one would then be taken to hold the new.

### decision every-way-round-has-a-limit
A cell's runs in a row that are ways round, the returns of its run sent back, and its runs in a row that only ask for calls each have a limit, at which the work waits for the owner.
- replaced: Counts of a work's loops recorded and reported, nothing limited, was on main; the owner changed it, because a loop that spends nothing but goes on round still needs a limit or a count.
- rejected: an agent proposed counting a cell's ways round apart for each cell that sends it back, so that one pair pushing back and forth stops it; the owner rejected it, because a send-back is itself a fault, and three abnormal ones are never let go unsolved.

### decision a-question-that-waits-holds-its-cell
A question its cell waits for holds the work at that cell, whether or not the cell applies, until its latest version is answered: no later cell runs, and the answer runs the cell again.
- replaced: A cell that asked a question that waits leaving out of what it dealt with whatever waited for the answer, so that it stayed new, was on main; the owner changed it, because the cell comes back by the question itself, which is what its stop flag was set for.

### decision a-work-no-cell-can-move-is-stalled
A work that waits only for what cells write, none of which will run, is stalled: shown halted with the walk's reason, worked out each time, it waits on no one, and nothing is recorded for it.
- rejected: an agent proposed counting a stalled work as waiting on the owner, with a card; the owner rejected it, because the owner can do nothing on the card, and a stall is a path the flow left open, to be closed where it is seen.

### decision a-stall-is-said-in-one-sentence
The page and the command line say why a work stalled in the one sentence the engine puts together from the parts in `[stalled]` of the flow's `waits.toml`.
- replaced: The page's own words for a stall and the command line's own was on main; the owner changed it, because the page and the command line then say it one way.

### decision what-a-run-wrote-may-wait-for-the-outside
Where only cells keep a precondition from holding, and the latest run of each said what it wrote waits for a block from outside the work (`waits_for`), the work waits for that source.
- rejected: an agent proposed showing such a work as waiting on the owner; the owner rejected it, because it waits for main to pass, not for the owner.

### decision a-paused-work-names-the-cell-it-would-run
For a paused work the walk names the cell it would run on being let on, and the page and the command line show that cell.
- rejected: an agent proposed that the page work the cell out from the states of the cells; the owner rejected it, because the next cell is the engine's to choose by the flow, and the page works out no state of its own.

### decision dependencies-reach-the-walk-as-a-datum
Whether the items a work depends on have merged reaches the walk only as versions of `dependencies`, which the engine writes from outside; no test of the flow reads another work.
- rejected: an agent proposed a named test that reads in memory whether the items the work depends on merged; the owner rejected it, because a replay of the work's records could then not reach the same decision.
