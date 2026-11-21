# The flow

The view of a flow's definition: its matrix, its flowchart and the panels of what is chosen on them.

### decision a-flowchart-beside-the-matrix
The Flow view draws the flow as a flowchart or as the matrix, switched on its line, the flowchart until the owner picks the matrix; one window shows what is chosen on either.
- replaced: The matrix alone stood on main; the owner changed it, because he wanted the branches, the ways back and what the data do drawn on the lines between the cells, as the old system's flowchart drew them.
- replaced: The matrix shown first stood on main; the owner changed it, because he asked for the flowchart first, giving no reason.

### decision a-cell-opens-everything-about-it
A chart names a cell by its id, the flowchart with its line and title too; everything else about it is in its panel, opened by choosing it.
- rejected: an agent proposed heaping in the chart's cells what was settled for each, and listing what was open under the chart; the owner rejected it, because he cannot look at the chart and read other text at once, and the chart is everything to him.

### decision the-flow-view-compares-no-versions
The Flow view shows the flow installed alone, with no switch to an earlier version and no list of what changed against another.
- replaced: A switch between the flow as it is and as it was, with a list of what changed, its rows struck through and coloured, stood on main; the owner changed it, because the comparison could not be read.

### decision a-datum-to-a-row-a-cell-to-a-column
The matrix sets each datum on a row and each cell on a column, in the order of the walk, with a mark where a cell and a datum cross.
- rejected: an agent proposed the cells as rows and the blocks as columns; the owner rejected it, because the names written upright across the top cannot be read.

### decision columns-as-wide-as-their-marks
A cell's column on the matrix is only as wide as its marks, its head the cell's id alone; the width this leaves is a column of words at the right, one line to a row.
- replaced: Columns headed by the cells' long titles stood on main; the owner changed it, because they left the middle of the chart wasted.
- rejected: an agent proposed lanes, one for each session, the data as lines from writer to reader; the owner rejected it, because the chart's trouble was its wasted middle, which lanes turned upside down rather than cured.

### decision the-lines-and-the-conditions-are-bands
Over the matrix's data stand a band of its lines, a dot under each cell of one, joined along the row, and a band of its conditions, a tick or a cross for a `when`, a square for a `requires`.
- replaced: The first word of each cell's line under its id, and its `when` as a diamond on the block's row far below, stood on main; the owner changed it, because the lines could hardly be told apart, and when a cell runs could be found only far down.
- rejected: an agent proposed a diamond for each `when` in the band of the conditions; the owner rejected it, because a tick or a cross says more plainly whether the cell runs.

### decision a-mark-says-what-the-cells-file-says
A cell's mark on a datum's row of the matrix says what the cell's file says of it: a ring where it watches it, the ring with the kind inside where it only reads it, a disc where it may write it.
- replaced: A band of triggers, each marked under what it runs, over a band of the data marked hollow for a read and solid for a write, the kind beside, stood on main; the owner changed it, because the rebuilt flow has no triggers, only what each cell watches, reads and writes.

### decision a-way-back-is-the-only-line-along-a-row
On the matrix the only line along a row of the data is a way back, from a later cell's write leftwards to each earlier cell that watches the datum, on its row or on its group's.
- replaced: A line from each cell that writes a trigger to each cell it runs, purple where it went back, stood on main; the owner changed it, because the marks' order already says where a write goes on, and the way back is what the owner looks for.

### decision a-condition-s-datum-is-one-row
On the matrix a datum a cell's `when` or `requires` names stands once, on its row in the band of the conditions, with the marks of what each cell's file says of it and its way back.
- replaced: Such a datum on a row of the conditions and again on one among the data stood on main; the owner changed it, because it should not be written in two places.

### decision the-corner-names-only-the-cells
The matrix's corner names the cells across, and no kind of row.
- replaced: A corner naming the rows as blocks and the columns as cells stood on main; the owner changed it, because the rows at the top are lines, not blocks.

### decision every-agent-is-a-band-of-its-own
On the matrix what `[every_agent]` names is a band of rows as `flow.toml` writes them, marked under each cell; the flowchart draws none of it, and the view's line names it.
- replaced: What every agent's cell reads drawn for each datum and each cell, and as tags of it on the flowchart, stood on main; the owner changed it, because those marks, all saying the same few things, buried each cell's own data.

### decision the-tools-are-a-band-of-rows
On the matrix each of the engine's tools is a row of a band of its own, marked under each cell that may call it; on the flowchart the view's line names them.
- replaced: The tools named once on the view's line, with no row of their own, stood on main; the owner changed it, because a call is a step of the work, and with the tests run by a call and not by a cell of their own, it was shown nowhere.

### decision one-rule-for-what-a-write-runs
The charts and the panels decide by one rule whether a cell's write can run another: it can, unless their `when`s name one block with values apart, the writer's keys on what it writes left out.
- replaced: An arrow back to every earlier cell that read what a later one wrote, between cells that cannot both run in one work too, stood on main; the owner changed it, because those arrows could no longer be read, and such a way back cannot happen.
- replaced: The flowchart leaving out the writer's own keys on what it writes, and the matrix keeping them, stood on main; the owner changed it, because the two charts could then say different things of one flow.

### decision the-flowchart-is-laid-out-as-the-old-systems
The flowchart is laid out and routed as the old system's was, by elkjs in layers from the top, its lines at rounded right angles arching where they cross, each label near where its line leaves.
- rejected: an agent proposed a flowchart with arrows it routed by hand; the owner rejected it, because its routing was far worse than the old system's.
- replaced: Square corners, lines crossing with no arch and the cells crowded together stood on main; the owner changed it, because the old chart's rounded corners, arches and room had been lost for nothing better.
- replaced: A line's label at its middle stood on main; the owner changed it, because he reads the labels to see a cell's ways out, and on a long line the middle is off the screen.

### decision the-main-path-stands-out
The flowchart draws the main path thick in the text's colour, laid straight down its middle, parting at a fork and joining after; a way back is thick in the accent, an arrow a write runs thin.
- replaced: Every line in one thin grey, the ways back thin and purple, stood on main; the owner changed it, because he could not see the main path.

### decision a-fork-is-one-node
Where the main path parts on a datum a `when` reads, the flowchart draws one fork named by the datum, each line out with its value, the ways back into it; a `requires` is a gate.
- replaced: A box of conditions before each cell, a `no` box parting into a `yes` box, stood on main; the owner changed it, because one yes or no was drawn as two boxes.

### decision data-arrows-over-the-layout
The flowchart draws the arrows a write runs only into and out of the cell chosen, or all while Data arrows is ticked, routed over a layout of the main path and the ways back alone.
- replaced: Every arrow a write runs drawn at once, laid out with the rest, stood on main; the owner changed it, because their winding routes were unreasonable, and no layout routes so many cleanly.

### decision the-flowchart-opens-at-its-own-size
The flowchart opens at 100%, its top in view, fitted to its width only where it is wider than the view; the rest is scrolled to.
- replaced: The whole chart fitted into the window stood on main; the owner changed it, because at about 62% the engineer's flow opened too small.

### decision a-cells-panel-shows-what-its-agent-is-sent
An agent cell's panel lists what its agent is sent, numbered in the order sent under program, System prompt and Message, each part by its key, its file and its text, in one column.
- rejected: an agent proposed a description of each field written for the owner apart from what the agent is told; the owner rejected it, because only by reading what the agent gets can he see what is missing.
- replaced: Struck rows, tasks before and after a change and sentences of explanation stood on main; the owner changed it, because the panel could not be read, the flow's files being clearer.
- rejected: an agent proposed grouping what is sent by when it is sent, with no numbers; the owner rejected it, because the numbers are what the parts are called by, and a system prompt and a message differ in kind.
- replaced: The form of a return as a part of its own between the system prompt and the message stood on main; the owner changed it, because the program makes the form a tool, and the tools stand first in every request.
- rejected: an agent proposed laying the panel in two columns, what goes in and what comes out, in blocks of a look of their own; the owner rejected it, because the window is narrow and the whole page has one look.
- replaced: A picker of how the cell is entered, showing the message each case would send, stood on main; the owner changed it, because that is a test's work, and a pick on the page changed nothing.

### decision the-form-is-drawn-as-a-tree
A cell's panel draws the form of a return, each of its three parts a tree in one column, a node folding at a press, each of several it holds one of in a box, an `or` line between.
- rejected: an agent proposed two columns, made-up names for the alternatives, a legend and lines down each node opened; the owner rejected it, because a box with an `or` line shows the alternatives plainly, as the old system did, and the lines were an eyesore.
- replaced: The form's JSON, with parts of its own for the calls and the outcome at the panel's end, stood on main; the owner changed it, because the form drawn as a tree makes those parts needless.
