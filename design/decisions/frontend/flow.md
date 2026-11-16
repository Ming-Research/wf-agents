# The flow

The view of a flow's definition: its chart and its panels.

### decision columns-as-wide-as-their-marks
On the flow's chart a cell's column starts only as wide as its marks: its head holds the cell's id and the first word
of its line, and the width this leaves is a column of words at the right, one line for each row.
Why: A column holds only marks, so a head wide enough for the cell's title leaves the middle of the chart empty and
the words of each row nowhere to stand; the title is shown for the cell under the pointer and in the cell's panel.
Drawing the cells in lanes, one for each session, was turned down, by judgment: with few lines most of the screen
would be blank.

### decision data-arrows-over-the-layout
The flowchart is laid out with the main path and the ways back alone; the arrows a write runs are routed over that
layout afterwards, those into and out of the cell chosen, or every one while the owner asks for them all.
Why: Laid out with the rest, each arrow shown or hidden would move the cells, and arrows not drawn would still bend
the main path, which is what the chart is for; by judgment.

### decision the-flowchart-opens-at-its-own-size
The flowchart opens at 100%, its top in view, fitted to its width only where it is wider than the view; the rest is
scrolled to.
Why: Fitted whole to the window, a flow as long as the engineer's opened at about 62%, where the owner could not read
its cells and labels.

### decision a-way-back-is-the-only-line-along-a-row
On the matrix the only line along a row of the data is a way back, drawn on the row of the name the earlier cell
watches: the datum's own row, or the group's where its file names the group; a write going right draws nothing. A cell
that watches a group has no mark on the group's blocks' rows, and a cell that reads a group is marked on each of its
blocks' rows with that block's kind.
Why: Each way back is then drawn once for each name watched and counted so, as the flowchart counts the names its
arrows carry; drawn on each block's row, a later cell that writes two blocks of a group an earlier cell watches would
draw one way back twice (no such pair is in the engineer flow, so this rests on judgment). A group read runs nothing as
one and each of its blocks has its own kind (design document `data-and-context`, 12, 17), so its marks are the blocks'.
A line for a write going right was turned down: the marks' order says it, and the way back is what the owner looks for.

### decision a-condition-s-datum-is-one-row
On the matrix a datum a key of a cell's `when` or `requires` names stands once, on its row in the band of the
conditions: the row's head is the datum's, its values and signs, and under each cell the condition's marks stand beside
those of what the cell's file says of the datum, with the way back along the row. A named test keeps a row of its own,
and the data it reads keep theirs among the data.
Why: Drawn in the band and again among the data, such a datum stood twice, its writer on one row and the cells that
wait for its value on the other (principle say-a-thing-once); a named test is no datum, and its data are many.

### decision the-way-back-is-in-the-accent
On the matrix a way back's line and arrow are drawn in the accent, and the arrow is drawn with the mark it points to, its
point on that mark's edge.
Why: The way back is what the owner looks for on the matrix, so it takes a strong mark (principle scarce-emphasis): in
the muted colour it read as one more of the rows' lines. A colour of its own was turned down as one more meaning to
learn; what else the matrix draws in the accent, what is chosen and the lines' rows, is told from it by shape, a way
back being a thick line along a datum's row with an arrow (judgment). Placed by the crossing's middle, the arrow reached
into the second of two marks.

### decision the-corner-names-only-the-cells
The matrix's corner names the cells across, and no kind of row.
Why: The rows are lines, conditions, tools, what `[every_agent]` names and data, a band's row tagged with its kind and
a datum's named with its values and signs, so a word for them all would be false of most of them or say again what the
rows say (principle say-a-thing-once).

### decision every-agent-is-a-band-of-its-own
On the matrix what `[every_agent]` names is a band of rows named as `flow.toml` writes them; under each column is
marked what the cell takes of the data a row stands for and what it writes of them, except what `[every_agent]` only
reads, which the row's words say; the data a `{each}_` name stands for have no rows of their own. A way back runs
along a row of the band as along any row, from a later cell's write of a datum it stands for to each earlier cell that
watches that datum, and is counted so. On the flowchart nothing of it is drawn coming in from outside; the view's line
names it. A cell's panel lists it first among the cell's inputs, by that key, its `do` said once in its own panel.
Why: Drawn for each datum and each cell, what every agent cell reads was 88 marks on the ten rows of the cells'
questions and words, the guide was split between a trigger row and a data row, and the flowchart drew ten tags of it,
all saying the same few things and burying each cell's own data. The guide's ring stays under each agent column: it
marks which cells a new guide runs, which nothing else on the matrix says. Leaving the band's ways back out, as the
data's rows they stood on were left out, was turned down: a cell that wrote a datum every agent's cell watches would
run the cells before it again unseen and uncounted.

### decision the-tools-are-a-band-of-rows
On the matrix each of the engine's tools is a row of a band of its own, marked under each cell that may call it.
Why: A call is a step of the work, which the chart must show (Article 7); keeping the tests a call rather than a cell of
their own (decision no-cell-runs-the-tests) left it shown nowhere but the view's line.

### decision the-view-line-names-what-the-chart-has-no-row-for
The view's line names the flow's tools and `[every_agent]` only while the flowchart is shown; while the matrix is shown
they are rows of its bands.
Why: Each thing is shown once in a view (Article 8), and the flowchart has no rows for them.

### decision one-rule-for-what-a-write-runs
Whether a cell's write can run another is decided by one rule wherever the page decides it, the flowchart's main path
and arrows and the matrix's and the panels' ways back alike: it can, but where their `when`s name one block with
different values, the writer's keys on a block it writes itself left out, as they may no longer hold once it is written.
Why: Decided twice, the flowchart and the matrix could say different things of one flow; R1 runs where `deep_needed` is
no and writes it, and its write runs D1, which runs where it is yes. No flow of the repository has a way back the two
rules differ on, so leaving the writer's own keys out of the ways back rests on judgment.
