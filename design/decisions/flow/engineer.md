# The engineer's flow

### decision weight-is-decided-in-the-work
How heavy a work runs is decided while it is done, by how hard it turns out: the research says whether deep research
is needed, and the test review whether more testing is. Nothing marks it when the item is made, and an item's level chooses
only the model.
Why: Size and kind are tangled: defects and needs come mixed, and what looks simple can turn serious halfway. Marking
it when the item was made was tried, and almost no item was marked, so all went the heavy way. A light flow of its own
for fixes and small features was turned down, since being flexible is not one heavy way and one light; and a half-made
short cut in the flow, since what is in the flow must be as good as it can be.

### decision the-research-hands-over-with-what-it-found-alone
When the research decides that the deep research is needed, its return holds nothing but what it found: no plan, no
question and no answer, in a dialogue or to the owner. `deep_needed` declares it (`with_nothing_but`), and a return that
holds anything else is sent back saying what.
Why: With yes the research runs no more, and the deep research takes the work, the research's dialogues and questions
with it (design document `data-and-context`, 27, 28), so what the research hands in beside the hand-over is left with
no cell that answers for it: a question it asked so, as one of its cases showed, stood while the owner had no cell to
speak to about it. Its task already said to write only what it found, and it asked all the same, so its words alone
did not stop it (principle locate-and-price-the-fix).

### decision acceptance-may-change
The conditions a work is accepted by may change while the work goes on.
Why: Fixing them once the first design is made is too strict.

### decision opinions-are-inputs
What someone has to say to a cell about what it did is written into a block that cell reads: the owner's words into
the cell's own words (data-and-context 35), a judging or doubting cell's points into the dialogue of that pair, in
which the cell answers them (data-and-context 36). The flow has no cell for fixing and none for checking a fix: what
was objected to is fixed by its cell running again, and the fix is looked at by the judging cell running again,
because what it judged changed.
Why: Going back is then the same thing as going on: an input changed. Cells of their own for the two needed rules of
their own for who fixes what a red CI before the merge found, and who checks that fix (principle
reviews-can-be-disputed).

### decision main-is-watched-by-the-check-alone
Main is watched only by the cell right before the merge, which checks that the work passes its whole CI with the
latest main taken in; a cell that changes the code takes main in as its run starts without reading it (decision
a-run-that-changes-the-code-takes-main-in). The merge only merges.
Why: The check is a cell of its own because a work ends when the merge has a record: nothing the merge wrote could
send the work back. Main watched by every cell was turned down, which would run a whole work again at each merge of
another.

### decision the-tree-runs-no-cell
The design tree is a datum each cell that reads the tree is given `last_and_latest`, the files that changed since the
version its session read, and runs no cell: a cell reads it, and none watches it.
Why: A changed tree watched would run the reading cell again, and every cell after it, for each commit that touches
the tree; read as a datum, it is read where the line goes on, by the cell that next enters the session.

### decision the-reviews-take-the-head-and-the-head-runs-the-check
Each review of the change takes `head`, the work's head, among the blocks of the change it is run by, and reads the
diff from main to it; `head`, not `code` or `tests`, runs the check, beside main and `asked`.
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

### decision the-check-waits-for-the-reviews
The check before the merge requires each review's yes beside its other gates, and runs again as the owner's answers
change whether every question is answered (`asked`), as well as on the work's head and on main.
Why: The watch of main writes main into every work, and before anything is judged the check's other gates hold: a
research that handed in no plan, or an implementation that handed in nothing, let main run the check on a branch no one
reviewed, holding the research's commits or nothing, whose CI passed and which merged. The old flow merged only after
its review approved. The answer to a question the check asks is asked in the implementation's questions, and the
implementation may have nothing to do for it: run only by what it writes, the check never looked again.

### decision the-better-way-is-looked-for-once
The cells that look for a better way run once a work, until a run of theirs has handed in, by `{cell}_handed_in` in
their condition, which the engine writes. They watch the plan and hand in a better way, a few sentences, or nothing, in
`better_found`, their dialogue with the plan's writer, who answers it there. A plan changed into another direction is
a new item, not a plan looked at again.
Why: In W11 the cell ran four times, once for each version the reviews sent back, and handed in nothing each time. In
W10 it changed the plan at later runs, run 31 after the implementation changed the acceptance and run 34 after the
research rewrote the plan, which running once gives up: later versions are the writer's answers to judges, whom the
reviews judge again, and that a better way is rarely found then is a judgment. The flag is written though the records
hold it, because a condition reads data alone, as the check's wait reads `asked` (decision
the-merge-gate-is-a-datum-the-engine-writes); a flow that gives a cell the condition in a work where the cell already
ran runs it once more, which is accepted. Watching `deep_needed`, whose first version runs a watcher once, was turned
down: the cell would be run by what it does not read. Writing the plan itself, it was a third writer beside the
research and the deep research, holding no dialogue with the plan's judges.

### decision the-implementation-waits-for-the-plans-verdicts
The implementation requires `plan_ok` and `tree_ok` to be yes.
Why: A pass runs every cell after a way back that something else runs (decision a-way-back-waits-for-the-pass), and
the implementation watches the plan: with no gate it would build a plan its review had just failed, which only the
order of the cells kept it from before (principle every-plan-is-reviewed-before-it-is-built).

### decision the-change-is-reviewed-in-three
The change is reviewed by three cells on one line: the code by the engineering rules, the tests and the evidence, and
the change against the design tree. Each numbers its questions one to a line and answers each by its number in its
verdict's why, saying what it checked and found; a question it did not check is a point.
Why: One review asking eleven things in one paragraph, W11's run 26, judged neither whether new tests were red before,
the tests it deleted, four of the five decisions nor time and speed. Its forks answered every question once told to
answer each, but passed one tucked into a long sentence with a yes and took the questions they had not checked on the
implementation's word: questions one to a line, with an unchecked question a point, are what the forks bear out. Three
cells rather than one numbered review rests on principle focused-judgment-alone, a judgment no fork compared. It costs
two more rounds at each version of the change: in W11 the review's first round cost $0.66 and a later one $0.11, and
that the second and third cells read the change from the session's cache is inferred. Answering every question does
not go against principle every-field-has-a-reader: what was checked is said, not made up.
