# The store

Where the records of the works are kept.

### decision tables-only-added-to
The tables the deciding code is given are only added to, each row with its place in the order of what happened, and no
two rows of a work at one place. There is no general store of events beside them.
Why: The deciding code is given a work's records as they were at a moment, and tables only added to give that with no
copy kept. A general store of events would be a mechanism with no second use (principles state-from-records,
generalise-at-the-second-use).

### decision a-migration-carries-each-row-whole
A migration that gives the rows kept before a value, such as a column made from what the rows' content says, makes the table anew and carries each row across whole with the value, in the same transaction, then takes the old table out; it never changes a row where it stands. The value is worked out by the code the engine uses as it writes the row, so that the two cannot disagree.
Why: Changing a row where it stands is turned down: the store says nothing that changes a row, and a test reads the code and the migrations for it (decision tables-only-added-to). The old table is taken out only once every row of it has been carried whole into the new one, in the same transaction.

### decision a-message-told-again-is-kept-under-a-second-name
What a later try of a round is told, where an earlier try of the round was told the same, is kept as a second name of
the file that try keeps it in, and no file of what a round was told is written over.
Why: Principle stored-content-is-kept-small: a try given again, as after one whose program took nothing in, is told what
the try before it was, whole. A small file of its own saying which try it repeats was turned down: every reader of the round, the count of
its tries among them, would have to follow it, where a second name is read as any file is, and rounds kept before read
as they did.
