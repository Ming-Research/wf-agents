# The flow's definition

What holds of the definition of every flow.

### decision the-table-of-earlier-ids-is-a-list-of-renumberings
The table of earlier ids is a list of renumberings, oldest first, each naming the cells whose id changed then, with
the id each had before; it is only added to, and a run recorded on a version is read through the renumberings after
that version's own.
Why: An id is given again whenever a cell is added or taken out (principle cell-numbering), so a cell's earlier ids
alone cannot say which cell an id was on a given version: a list of each cell's earlier ids reads a round of an id
since given to another cell as that other cell. A table of the last renumbering alone was turned down: a version the
engine never started on would leave its renumbering out of what a work on an earlier one is read through.
