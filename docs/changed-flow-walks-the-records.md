# A changed flow walks the records

How a change to a flow reaches the works under way. The walk already goes over a work's records on a changed graph,
and a record is found by its cell through the table of earlier ids; a cell's specification as a datum it watches and
the walk without acting are not built.

1. A changed flow is installed by walking the records of each work that has not ended on the new graph: a cell that
   did not change is passed, a new cell has no record and runs when the walk reaches it, and what a cell taken out
   wrote stays in the item. A work that ended stays on the version it ended on.

2. The specification of a cell is a datum that cell watches. A change of it is a write from outside, and whether it
   matters to the works under way, so that they run the cell again, the owner says when approving the change.

3. Walking without acting, before the change is approved, shows for each work from which cell it would run again.

4. A record is found by its cell, and a datum made for a cell by the cell it is made for, through the table of
   earlier ids (principle cell-numbering).

Records are kept by cell and by the versions each run started from, not by a step in a course taken, so they still fit
a graph that has changed; and a changed cell is one more datum it watches that changed (principle
flow-change-spares-works).
