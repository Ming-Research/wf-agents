# The flow

The view of a flow's definition: its chart and its panels.

### decision columns-as-wide-as-their-marks
On the flow's chart a cell's column is only as wide as its marks: its head holds the cell's id and the first word of
its line, and the width this leaves is a column of words at the right, one line for each block.
Why: A column holds only marks, so a head wide enough for the cell's title leaves the middle of the chart empty and
the words of each row nowhere to stand; the title is shown for the cell under the pointer and in the cell's panel.
Drawing the cells in lanes, one for each session, was turned down: with few lines most of the screen would be blank.

### decision compared-with-the-flow-as-committed
The page of a running engine compares the flow's files with the flow as the commit checked out in their repository
has it, and a flow in no repository with nothing.
Why: A change to the flow is first an edit of its files that is not committed, and the owner reviews it on the page
(Article 7). Comparing with what the engine read when it started was turned down: the page is built into the program,
so the engine is started again for a change of the page and would then list nothing. So was comparing with the
remote's main branch, which would list backwards a change merged there that the clone does not have yet. Both rest on
judgment.
