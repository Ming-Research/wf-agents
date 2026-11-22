# A changed flow walks the records

How a change to a flow reaches the works under way. The walk already goes over a work's records on a changed graph, a
record is found by its cell through the table of earlier ids, and a line whose session began with another system prompt
or form begins a new one (4).

1. A changed flow is installed by walking the records of each work that has not ended on the new graph: a cell that
   did not change is passed, a new cell has no record and runs when the walk reaches it, and what a cell taken out
   wrote stays in the item. A work that ended stays on the version it ended on.

2. A cell a work has already passed is not run again because its specification changed: the change applies to the
   cells the work has not passed yet (principle flow-change-spares-works).

3. A record is found by its cell, and a datum made for a cell by the cell it is made for, through the table of
   earlier ids (principle cell-numbering).

4. A line's session keeps the system prompt and the form of return it began with. So a changed system prompt or form
   reaches a work under way as a new session of each line whose kept ones differ from what a new session would begin
   with now: the line begins it at its next round, as for a change of model, and what was handed in still counts
   (decision a-changed-system-prompt-or-form-begins-a-new-session).

Records are kept by cell and by the versions each run started from, not by a step in a course taken, so they still fit
a graph that has changed; and a cell a work has passed keeps its record when the cell changes (principle
flow-change-spares-works).
