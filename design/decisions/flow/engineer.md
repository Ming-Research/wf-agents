# The engineer's flow

### decision weight-is-decided-in-the-work
How heavy a work runs is decided while it is done, by how hard it turns out: the research says whether deep research
is needed, and the review whether more testing is. Nothing marks it when the item is made, and an item's level chooses
only the model.
Why: Size and kind are tangled: defects and needs come mixed, and what looks simple can turn serious halfway. Marking
it when the item was made was tried, and almost no item was marked, so all went the heavy way. A light flow of its own
for fixes and small features was turned down, since being flexible is not one heavy way and one light; and a half-made
short cut in the flow, since what is in the flow must be as good as it can be.

### decision acceptance-may-change
The conditions a work is accepted by may change while the work goes on.
Why: Fixing them once the first design is made is too strict.

### decision opinions-are-inputs
What someone has to say to a cell about what it did is written into a block that cell reads: the owner's words into
the record of what is said to the work (data-and-context 30 to 38), a judging cell's findings into a block for that
pair. The flow has no cell for fixing
and none for checking a fix: what was objected to is fixed by its cell running again, and the fix is looked at by the
judging cell running again, because what it judged changed.
Why: Going back is then the same thing as going on: an input changed. Cells of their own for the two needed rules of
their own for who fixes what a red CI before the merge found, and who checks that fix (principle
reviews-can-be-disputed).

### decision main-is-an-input-of-the-check-alone
Main is an input only of the cell right before the merge, which checks that the work passes its whole CI with the
latest main taken in; a cell that changes the code takes main in as its run starts without reading it (decision
a-run-that-changes-the-code-takes-main-in). The merge only merges.
Why: The check is a cell of its own because a work ends when the merge has a record: nothing the merge wrote could
send the work back. Main as an input of every cell was turned down, which would run a whole work again at each merge
of another.

### decision the-tree-runs-no-cell
The design tree is a datum each cell that reads the tree is given `last_and_latest`, the files that changed since the
version its session read, and runs no cell; of what comes from outside the work, only main and `asked` run a cell, the
check before the merge.
Why: A changed tree run as a trigger would run the reading cell again, and every cell after it, for each commit that
touches the tree; given as a datum, it is read where the line goes on, by the cell that next enters the session.

### decision the-check-waits-for-the-review
The check before the merge requires the review's yes beside its other gates, and runs again as the owner's answers
change whether every question is answered (`asked`), as well as on the change and on main.
Why: The watch of main writes main into every work, and before anything is judged the check's other gates hold: a
research that handed in no plan, or an implementation that handed in nothing, let main run the check on a branch no one
reviewed, holding the research's commits or nothing, whose CI passed and which merged. The old flow merged only after
its review approved. The answer to a question the check asks reaches the last cell with a trigger of an answer, the
implementation, which may have nothing to do: run only by what it writes, the check never looked again.

### decision every-commit-is-reviewed-before-the-merge
The work names the commit it hands in only in `code` and `tests`, the blocks that name the head of its branch, which are
both of the group that runs the review; the check passes only a head they name, so each commit it passes ran the review,
which comes before it in the walk and whose yes it requires, before the check. The testing's commits are reviewed so, by
the review run again for its `tests`; no cell of its own reviews them, and the testing does not come before the review.
Why: The testing commits its tests after the review's yes, and a commit named only in an answer, as to the check, which
runs the check and not the review, reached the merge with no review of it. A second review after the testing was turned
down: the review already runs on each new version of `tests`, and a second cell would judge the same change by the same
standard twice. Testing before the review was turned down: the review says whether the change needs it.
