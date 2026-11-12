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
Why: Fixed after the first design, they were too strict.

### decision opinions-are-inputs
What someone has to say to a cell about what it did is written into that cell's block of opinions, which is one of
its inputs. The flow has no cell for fixing and none for checking a fix: what was objected to is fixed by its cell
running again, and the fix is looked at by the judging cell running again, because what it judged changed.
Why: Going back is then the same thing as going on: an input changed. Cells of their own for the two needed rules of
their own for who fixes what a red CI before the merge found, and who checks that fix (principle
reviews-can-be-disputed).

### decision main-is-read-by-the-check-alone
Main is an input only of the cell right before the merge, which checks that the work passes its whole CI with the
latest main taken in; the merge only merges.
Why: The check is a cell of its own because a work ends when the merge has a record: nothing the merge wrote could send
the work back. Every cell reading main was turned down, which would run a whole work again at each merge of another.
