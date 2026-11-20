# The flow

What every flow's definition holds, whichever flow it is.

### decision every-agent-takes-is-the-flows
What every agent cell of a flow reads beside what its own file names, such as every cell's questions and words, is named
once in the flow's `flow.toml` (`[every_agent]`), each with its kind, and given each agent cell first; a datum a cell
names itself it takes as its own file names it. The engine holds no list of it, and no cell's file repeats it.
Why: What every cell takes is part of the flow, which the owner reads in its files and on its chart (Article 7); written
in each cell's file it would be one thing in many places, and held in the engine's code, one the flow does not show
(principle one-thing-one-place).

### decision a-datum-of-a-cell-s-own-is-made-where-a-cell-names-it
A datum each cell or each line has its own of, such as a cell's questions, is made from its file beside the flows only
for a cell or a line whose name a cell names it by, among what that cell watches, reads or writes or in its condition or
preconditions; `{each}_` in what every agent cell takes stands for those made.
Why: Made for every cell, a cell that may not ask would have questions of its own that no cell writes and that run
nothing, each a block with no writer (principle every-field-has-a-reader).

### decision a-cell-names-a-block-once
A cell's file takes each block by one name among what it watches and reads, the block's own or one group's; a block
taken by two, by itself and through a group or through two groups, is refused as the flow compiles.
Why: Taken by two names, what the block does for the cell is said twice, and where one is watched and the other read the
two contradict each other. Drawing both on the page ("watches G, latest; reads latest") was turned down: it shows the
contradiction and leaves it standing.

### decision the-table-of-earlier-ids-is-a-list-of-renumberings
The table of earlier ids, `earlier_ids` in a flow's `flow.toml`, is a list of renumberings, oldest first, each naming
the cells whose id changed then, by the id each had, with the id it takes, and the cells taken out, by the id each had,
a stage's last among them; it is only added to, and what was recorded on a version is read through the renumberings
after that version's own. A cell taken out reads as a cell no flow has. A renumbering is refused as the flow compiles
where it gives an id of a stage it moves or takes out a cell of, numbered at most the highest it moves or takes out
there, without moving or taking out the cell that had that id: the stage numbers its cells with no gap, so it had
every id up to that one before. What the compiler cannot see is checked as the engine starts (decision
a-table-that-misreads-a-kept-version-is-refused).
Why: An id is given again whenever a cell is added or taken out (principle cell-numbering), so a cell's earlier ids
alone cannot say which cell an id named on a given version, and a cell taken out whose id another took would read as
that other; a stage's last cell taken out leaves its id to be given again. A table of the last renumbering alone was
turned down: a version the engine never started on would leave its renumbering out of what a work on an earlier one is
read through.

### decision a-datum-made-for-a-cell-follows-its-cell
A datum each cell has its own of, made by a file of `data/of_a_cell/`, is read through a renumbering as the datum made
for the id its cell takes, as the cell is; no table names it.
Why: Its name is made from its cell's id, so a renumbering renames it as surely as the cell: read under its old name, a
renumbered cell's words, questions and hand-in would be data taken out, and what was new to the cell no longer would
be. Listing such data in the table was turned down as a second copy of what the file that makes them says (principle
one-thing-one-place).
