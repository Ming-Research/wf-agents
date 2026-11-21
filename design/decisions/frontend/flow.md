# The flow

The view of a flow's definition: its matrix, its flowchart, and the panels for what is chosen on them.

### decision a-flowchart-beside-the-matrix
The Flow view draws the flow as a flowchart or as the matrix, with a switch on its line, the flowchart until the owner picks the matrix; one window shows what is chosen on either.
- replaced: The matrix alone was on main; the owner changed it, because he wanted the branches, the ways back and what the data do drawn on the lines between the cells, as the old system's flowchart drew them.
- replaced: The matrix shown first was on main; the owner changed it, because he asked for the flowchart first, giving no reason.

### decision a-cell-opens-everything-about-it
A chart names a cell by its id, and the flowchart also by its line and title; everything else about the cell is in its panel, which opens when the owner chooses the cell.
- rejected: an agent proposed piling into the chart's cells what was settled for each, and listing what was open under the chart; the owner rejected it, because he cannot look at the chart and read other text at once, and the chart is everything to him.

### decision the-flow-view-compares-no-versions
The Flow view shows only the installed flow, with no switch to an earlier version and no list of what changed against another.
- replaced: A switch between the flow as it is and as it was, with a list of what changed, its rows struck through and coloured, was on main; the owner changed it, because the comparison could not be read.

### decision a-datum-to-a-row-a-cell-to-a-column
The matrix puts each datum on a row and each cell in a column, in the order of the walk, with a mark where a cell and a datum cross.
- rejected: an agent proposed the cells as rows and the blocks as columns; the owner rejected it, because names written vertically across the top cannot be read.

### decision columns-as-wide-as-their-marks
A cell's column on the matrix is only as wide as its marks, headed by the cell's id alone; the width this frees is a column of words at the right, one line per row.
- replaced: Columns headed by the cells' long titles was on main; the owner changed it, because they wasted the middle of the chart.
- rejected: an agent proposed lanes, one for each session, with the data as lines from writer to reader; the owner rejected it, because the chart's problem was its wasted middle, which lanes turned upside down rather than cured.

### decision the-lines-and-the-conditions-are-bands
Above the matrix's data are a band of its lines, a dot under each cell of a line, joined along the row, and a band of its conditions, a tick or a cross for a `when` and a square for a `requires`.
- replaced: The first word of each cell's line under its id, and its `when` as a diamond on the block's row far below, was on main; the owner changed it, because the lines could hardly be told apart, and when a cell runs could be found only far down.
- rejected: an agent proposed a diamond for each `when` in the band of the conditions; the owner rejected it, because a tick or a cross says more plainly whether the cell runs.

### decision a-mark-says-what-the-cells-file-says
A cell's mark on a datum's row of the matrix shows what the cell's file says of the datum: a ring if it watches it, a ring with the kind inside if it only reads it, a disc if it may write it.
- replaced: A band of triggers, each marked under what it runs, above a band of the data marked hollow for a read and solid for a write, with the kind beside, was on main; the owner changed it, because the rebuilt flow has no triggers, only what each cell watches, reads and writes.

### decision a-way-back-is-the-only-line-along-a-row
On the matrix, the only line along a data row is a way back: from a later cell's write leftwards to each earlier cell that watches the datum, on the datum's row or on its group's.
- replaced: A line from each cell that writes a trigger to each cell it runs, purple where it went back, was on main; the owner changed it, because the order of the marks already shows where a write leads, and the way back is what the owner looks for.

### decision a-condition-s-datum-is-one-row
On the matrix, a datum that a cell's `when` or `requires` names appears once, on its row in the band of the conditions, with the marks of what each cell's file says of it, and its way back.
- replaced: Such a datum on a row of the conditions and again on a row among the data was on main; the owner changed it, because it should not be written in two places.

### decision the-corner-names-only-the-cells
The matrix's corner names the cells across the top, and no kind of row.
- replaced: A corner naming the rows as blocks and the columns as cells was on main; the owner changed it, because the rows at the top are lines, not blocks.

### decision every-agent-is-a-band-of-its-own
On the matrix, what `[every_agent]` names is a band of rows as `flow.toml` writes them, marked under each cell; the flowchart draws none of it, and the view's line names it.
- replaced: What every agent's cell reads, drawn for each datum and each cell and as tags on the flowchart, was on main; the owner changed it, because those marks, all saying the same few things, buried each cell's own data.

### decision the-tools-are-a-band-of-rows
On the matrix, each of the engine's tools is a row in a band of its own, marked under each cell that may call it; on the flowchart, the view's line names them.
- replaced: The tools named once on the view's line, with no row of their own, was on main; the owner changed it, because a call is a step of the work, and since the tests are run by a call and not by a cell of their own, that step was shown nowhere.

### decision one-rule-for-what-a-write-runs
The charts and the panels use one rule for whether a cell's write can run another: it can, unless their `when`s name one block with no value in common, leaving out the writer's keys on what it writes.
- replaced: An arrow back to every earlier cell that read what a later one wrote, even between cells that cannot both run in one work, was on main; the owner changed it, because those arrows could no longer be read, and such a way back cannot happen.
- replaced: The flowchart leaving out the writer's own keys on what it writes, and the matrix keeping them, was on main; the owner changed it, because the two charts could then say different things about one flow.

### decision the-flowchart-is-laid-out-as-the-old-systems
The flowchart is laid out and routed as the old system's was: by elkjs, in layers from the top, its lines at rounded right angles, arching where they cross, each label near where its line starts.
- rejected: an agent proposed a flowchart with arrows it routed by hand; the owner rejected it, because its routing was far worse than the old system's.
- replaced: Square corners, lines crossing with no arch and the cells crowded together was on main; the owner changed it, because the old chart's rounded corners, arches and room had been lost for nothing better.
- replaced: A line's label at its middle was on main; the owner changed it, because he reads the labels to see a cell's ways out, and on a long line the middle is off the screen.

### decision the-main-path-stands-out
The flowchart draws the main path thick in the text's colour, straight down its middle, splitting at a fork and joining after it; a way back is thick in the accent, and an arrow a write runs is thin.
- replaced: Every line in one thin grey, the ways back thin and purple, was on main; the owner changed it, because he could not see the main path.

### decision a-fork-is-one-node
Where the main path splits on a datum a `when` reads, the flowchart draws one fork named by the datum, each line out with its value, and the ways back into it; a `requires` is a gate.
- replaced: A box of conditions before each cell, with a `no` box branching into a `yes` box, was on main; the owner changed it, because one yes or no was drawn as two boxes.

### decision data-arrows-over-the-layout
The flowchart draws the arrows a write runs only into and out of the chosen cell, or all of them while Data arrows is ticked, routed over a layout of the main path and the ways back alone.
- replaced: Every arrow a write runs drawn at once, laid out with the rest, was on main; the owner changed it, because their winding routes were unreasonable, and no layout routes so many cleanly.

### decision the-flowchart-opens-at-its-own-size
The flowchart opens at 100% with its top in view, shrunk to fit its width only where it is wider than the view; the owner scrolls to the rest.
- replaced: The whole chart fitted into the window was on main; the owner changed it, because at about 62% the engineer's flow opened too small.

### decision a-cells-panel-shows-what-its-agent-is-sent
An agent cell's panel lists in one column what its agent is sent, numbered in the order sent, under program, System prompt and Message, each part with its key, its file and its text.
- rejected: an agent proposed a description of each field, written for the owner, separate from what the agent is told; the owner rejected it, because only by reading what the agent gets can he see what is missing.
- replaced: Struck-through rows, tasks before and after a change, and sentences of explanation was on main; the owner changed it, because the panel could not be read, and the flow's files were clearer.
- rejected: an agent proposed grouping what is sent by when it is sent, with no numbers; the owner rejected it, because the parts are called by their numbers, and a system prompt and a message are different in kind.
- replaced: The form of a return as a separate part between the system prompt and the message was on main; the owner changed it, because the program turns the form into a tool, and the tools come first in every request.
- rejected: an agent proposed laying out the panel in two columns, what goes in and what comes out, in blocks with a look of their own; the owner rejected it, because the window is narrow and the whole page has one look.
- replaced: A picker for how the cell is entered, showing the message each case would send, was on main; the owner changed it, because that is a test's job, and picking on the page changed nothing.

### decision the-form-is-drawn-as-a-tree
A cell's panel draws each of the three parts of a return's form as a tree in one column; a press folds a node, and where a node holds one of several, each is in a box with an `or` line between.
- rejected: an agent proposed two columns, made-up names for the alternatives, a legend, and lines down each opened node; the owner rejected it, because a box with an `or` line shows the alternatives plainly, as the old system did, and the lines were an eyesore.
- replaced: The form's JSON, with separate parts for the calls and the outcome at the end of the panel, was on main; the owner changed it, because drawing the form as a tree makes those parts needless.
