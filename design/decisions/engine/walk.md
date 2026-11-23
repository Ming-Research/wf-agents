# The walk

What decides, from a work's records and the flow, which cell runs next, and what holds a work where it stands.

### decision the-walk-goes-from-the-first-cell
Each time something happens to a work, the walk goes through the cells from the first, enters the first that something new runs, whoever wrote it, and does not store where the work stands.
- replaced: Rows back drawn as routes of the state chart, with the rules for sending a work back kept outside the chart, was on main; the owner changed it, because then a refused approval is only something made stale and a new walk from the start.
- rejected: an agent proposed a work as its products alone, with no cells and no position, its next step worked out from what is missing or stale; the owner rejected it, because he shapes the flow with cells, as by adding a review after the design, and products would carry the state machine anyway.
- rejected: an agent proposed that a cell that runs again makes every cell after it stale; the owner rejected it, because only a changed input a cell depends on makes it stale, and an output that did not change runs nothing after it.
- replaced: A cell run only by later cells' writes waiting while a later cell before the next gate was due was on main; the owner changed it, because P1 and P2 reviewed a plan R2 had a better way for, and a second look costs a fraction of a first (data: `research/a-review-looks-again-for-less.md`).
- rejected: the owner proposed running as a batch every cell that had something new when the batch began, then the next batch from what they wrote; the owner rejected it, because the records alone give the same with no batch recorded, and what a cell of the pass makes new runs in that pass.
- rejected: an agent proposed that the maker answers only once every review of what it made has seen its latest version; the owner rejected it, because the rules have only reads, writes and the runs that reads bring, and a condition made for reviews would be a new mechanism.

### decision conditions-and-preconditions
The walk passes a cell whose condition does not hold, and what it wrote still counts until a newer version; a precondition holds the work at its cell, is worked out at each walk and is never stored.
- rejected: an agent proposed that what a cell wrote no longer counts while the walk passes it; the owner rejected it, because a cell that is not run leaves what it wrote valid, and only a veto, such as his refused approval, voids something.

### decision meaning-told-by-the-writer
A choice or a list of names changes its meaning when its value changes; prose changes it only when its writer says the change matters, and a version said not to matter is kept and runs no cell.
- rejected: an agent proposed taking every new text of a block as a change that runs the cells after it; the owner rejected it, because a mechanical comparison would run everything after a reworded text again.
- replaced: Writing a version handed in as changing nothing in meaning over the version it mends was on main; the owner changed it, because the cells that had the old version would then be taken to have the new one.

### decision every-way-round-has-a-limit
Each of these has a limit at which the work waits for the owner: a cell's runs in a row that are ways round, the returns of its run sent back, and its runs in a row that only ask for calls.
- replaced: Recording and reporting counts of a work's loops, with nothing limited, was on main; the owner changed it, because a loop that spends nothing but keeps going round still needs a limit or a count.
- rejected: an agent proposed counting a cell's ways round separately for each cell that sends it back, so that one pair pushing back and forth stops it; the owner rejected it, because a send-back is itself a fault, and three abnormal send-backs are never let go unsolved.

### decision a-question-that-waits-holds-its-cell
A question its cell waits for holds the work at that cell, whether or not the cell applies, until its latest version is answered: no later cell runs, and the answer runs the cell again.
- replaced: A cell that asked a question it waits for leaving whatever waited for the answer out of what it dealt with, so that it stayed new, was on main; the owner changed it, because the question itself brings the cell back, which is what its stop flag was set for.

### decision a-work-no-cell-can-move-is-stalled
A work that waits only for what cells write, none of which will run, is stalled: it is shown halted with the walk's reason, worked out each time; it waits on no one, and nothing is recorded for it.
- rejected: an agent proposed counting a stalled work as waiting on the owner, with a card; the owner rejected it, because the owner can do nothing on the card, and a stall is a path the flow left open, which is to be closed where it is seen.

### decision a-stall-is-said-in-one-sentence
The page and the command line say why a work stalled in the one sentence the engine puts together from the parts in `[stalled]` of the flow's `waits.toml`.
- replaced: The page and the command line each having their own words for a stall was on main; the owner changed it, because then the page and the command line say it in one way.

### decision what-a-run-wrote-may-wait-for-the-outside
When only cells keep a precondition from holding, and the latest run of each said that what it wrote waits for a block from outside the work (`waits_for`), the work waits for that source.
- rejected: an agent proposed showing such a work as waiting on the owner; the owner rejected it, because it waits for main to pass, not for the owner.

### decision a-paused-work-names-the-cell-it-would-run
For a paused work, the walk names the cell the work would run once it is let go on, and the page and the command line show that cell.
- rejected: an agent proposed that the page work the cell out from the states of the cells; the owner rejected it, because the next cell is the engine's to choose by the flow, and the page works out no state of its own.

### decision dependencies-reach-the-walk-as-a-datum
The walk learns whether the items a work depends on have merged or been cancelled only from versions of `dependencies`, which the engine writes from outside; no test of the flow reads another work.
- rejected: an agent proposed a named test that reads in memory whether the items the work depends on merged; the owner rejected it, because then a replay of the work's records could not reach the same decision.

### decision a-run-records-the-version-its-cell-last-worked-from
The cause a run begins for records, beside the version the cell consumed, the version it last worked from, which the walk works out as the run begins and the page shows; a cause kept before has none.
- rejected: an agent proposed working the version out on the page, as it is drawn, by today's flow; the owner rejected it, because after the flow changes a past run could show a number different from the one its message had, the past being read by today's flow and not as it ran.
