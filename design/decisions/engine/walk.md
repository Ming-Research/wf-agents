# The walk

What decides, from a work's records and the flow, which cell runs next.

### decision conditions-and-preconditions
A cell whose condition is not met is skipped, and what it wrote before still counts: the blocks it wrote stay as they
are, and nothing but a newer write changes them; only a test of what stands leaves them out. A gate is no cell but a
precondition of the cell it guards, worked out at every walk and never stored.
Why: A cell skipped this time has not been undone: a review that finds the tests enough does not make the tests
written before worthless. A gate drawn as a cell needed a record of what the work was waiting for, and care to raise
and withdraw that record at the right moments.

### decision every-way-round-has-a-limit
Each thing the flow lets repeat has a limit: how many runs of a cell in a row are ways round, how often what a run
returns is sent back, how many calls a run makes; at a limit the work waits for the owner. A run is a way round when
each of its reasons is a block whose meaning a cell after it last changed. A run with a reason that a person, the
outside or a cell before it wrote starts the count anew, and so does the owner's letting a cell go on.
Why: In any circle of cells that send each other round, the first of them in the order runs only for what the later
ones wrote, so its count grows whatever the others' reasons are, and one count bounds them all. A person's opinion is
the owner steering and no circle, so it starts the count anew, and the owner is not asked to let go on what the owner
sent back (principle no-unseen-spending-loop).

### decision meaning-told-by-the-writer
A block that is a choice or a list of names changes its meaning when its value does, whoever writes it. Prose changes
its meaning when its writer says so: with each write the cell says, with a line of why, whether the change matters to
the cells that read the block, compared with the version at which the meaning last changed. Prose a person writes
always changes the meaning, and so does what the outside writes, unless whoever approved that write said it does not.
A cell that says a change does not matter without having seen what it wrote over is not taken at its word. What a
change that does not matter leaves undone is in the design of data and context.
Why: An agent never writes the same text twice, so judging prose by its text would run every later cell again after
every rerun. A second agent to check the answer, at a round for every change, was turned down, and so was a count that
goes up at each change of meaning, which would leave which version a write is compared with to whoever counts
(Article 3).