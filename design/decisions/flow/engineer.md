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
the cell's own words (data-and-context 35), a judging cell's findings into a block for that pair. The flow has no
cell for fixing
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
version its session read, and runs no cell; of what comes from outside the work, a cell is run by what it names in its
`triggered_by`, as the flow's triggers and data declare, and none names the tree.
Why: A changed tree run as a trigger would run the reading cell again, and every cell after it, for each commit that
touches the tree; given as a datum, it is read where the line goes on, by the cell that next enters the session.

### decision the-review-takes-the-head-and-the-head-runs-the-check
The review takes `head`, the work's head, among the blocks of the change it is run by, and reads the diff from main to
it; `head`, not `code` or `tests`, runs the check, beside main and `asked`.
Why: Whatever a round commits moves the head, under whatever block it hands in or none, as the fix of W9 did, handed in
under a block the review was not run by, so the review runs on every change it has not seen and the check on every
head; run by `code` and `tests`, the check ran on what a block said and not on what lands. The check holds the head to
the review (decision the-check-holds-the-head-to-what-the-review-saw).

### decision the-check-is-answered-by-a-commit-or-a-question
The implementation answers a red check by fixing it and committing, or, where it judges the red not this change's to
fix, by asking the owner; no block holds its answer.
Why: The check reads no words: the fix runs it again through the work's head, and the owner's answer through `asked`
(decision an-act-is-answered-by-what-runs-it). A block of the answer, which no cell read, ran the check on its words,
and one that changed nothing left the work at the red check until the owner spoke to a cell.

### decision the-check-waits-for-the-review
The check before the merge requires the review's yes beside its other gates, and runs again as the owner's answers
change whether every question is answered (`asked`), as well as on the work's head and on main.
Why: The watch of main writes main into every work, and before anything is judged the check's other gates hold: a
research that handed in no plan, or an implementation that handed in nothing, let main run the check on a branch no one
reviewed, holding the research's commits or nothing, whose CI passed and which merged. The old flow merged only after
its review approved. The answer to a question the check asks is asked in the implementation's questions, and the
implementation may have nothing to do for it: run only by what it writes, the check never looked again.
