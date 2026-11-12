# The walk

What decides, from a work's records and the flow, which cell runs next.

### decision conditions-and-preconditions
A cell whose condition is not met is skipped, and what it wrote before still counts: the blocks it wrote stay as they
are, and nothing but a newer write changes them; only a test of what stands leaves them out. A gate is no cell but a
precondition of the cell it guards, worked out at every walk and never stored.
Why: A cell skipped this time has not been undone: a review that finds the tests enough does not make the tests written
before worthless. A gate drawn as a cell needed a record of what the work was waiting for, and care to raise and
withdraw that record at the right moments.

### decision every-way-round-has-a-limit
Each thing the flow lets repeat has a limit: how many runs of a cell in a row are ways round, how often what a run
returns is sent back, how many calls a run makes; at a limit the work waits for the owner. A run with a reason that a
person, the outside or a cell before it wrote starts the count of ways round anew, and so does the owner's letting a
cell go on.
Why: In any circle of cells that send each other round, the first of them in the order runs only for what the later
ones wrote, so its count grows whatever the others' reasons are, and one count bounds them all. A person's opinion is
the owner steering and no circle (principle no-unseen-spending-loop).
