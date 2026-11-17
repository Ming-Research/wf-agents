# The context manager

What a round of an agent's cell is sent as the cell is entered, and what its session holds.

### decision a-fill-counts-where-its-program-had-the-message
What a try's message fills is recorded before its program is given the message, and a session holds it only where the round reached the model, as the tokens the round took say, or its last try's program said, as its stream began, that it had taken its message in. What the round failed of says nothing of this. Whether a round begins its session is counted the same way.
Why: Recorded after the try, a try cut off or an engine stopped during it would leave the record short of what the session holds. The program writes the message into the session before it calls the service (decision `the-engine-checks-the-outside-itself`), so a round tried again after it would put a second copy there; but what a round failed of does not tell a crash before the program read its input from one after, and a message referred to and lacking leaves the agent without it, which costs more than tokens.

### decision a-compaction-empties-the-session-at-its-round
A session compacted during a round holds, for the rounds after it, only what that round handed in: from where the compaction is recorded as the session reopened (decision a-compaction-reopens-the-session-under-its-id), the message the compaction's hook gives the session again within the round not counted as held.
Why: A compaction leaves only a summary of what the session was sent, which is no version of anything; the hook's message may be compacted again before the round ends, so what it gave is sent again rather than taken as held.

### decision a-datum-held-and-never-worked-from-is-named-by-its-version
Of a datum the session holds that the cell never worked from, as a cell entered after another of its line that read or wrote it, `held_new`'s line is sent: its name and the version the session holds.
Why: All of it is new to the cell, and the system prompt tells the agent that a block the session holds already comes as a line that says so; with nothing sent, a cell such as the implementation after the plan was told to implement a block its message never named. Sending it whole again was turned down for what it costs (judgment, not measured).

### decision last-worked-from-a-datum-that-runs-no-cell
Of a datum that does not run the cell, the version the cell last worked from is the latest written before its last finished run's last round began; of any datum, a version a run of the cell wrote is one it worked from.
Why: Only a datum that runs a cell is consumed (design document `data-and-context`, 8), and every round is filled with the latest of each datum it is given, so that is the version the run worked from; a cell's own version is no change to it (design document `data-and-context`, 9).

### decision what-changed-in-the-tree-is-from-what-the-session-read
Of a datum whose versions the carrier compares itself, as the design tree's by git, what changed is what the carrier says, sent whatever its length, and from the version the session holds where the cell worked from none.
Why: Such a datum's content, the tree's commit and id, says nothing an agent reads, while the files that changed tell it what to read again; and it stands for what was read into the session, so what the session read is what a change is from, as the cells that are given the tree say.

### decision a-put-in-is-held-from-the-round-it-reached
A version put into a round counts as held by the session only from a round before the one entered, put in while it ran, and only where that round reached the model.
Why: A version is put into a round as it begins, as one its return would have written over; counted held for that round itself, it would never be filled while the message names it, and the next return would write over it unseen.

### decision a-version-written-over-is-given-with-why
Where a return was sent back for writing over a version it never saw, that version is given once: by the context where the round's context gives it, as a block the cell watches or reads is given by its kind and one it only writes by its latest where the session lacks it (document data-and-context, 21); else with why, in the words a datum given `latest` is given in, as a version written while the round ran.
Why: With no tool to read the blocks, nothing else shows the agent what it is to write over; given in both places, the session would hold it twice.

### decision a-version-is-held-by-what-it-is
A version is held by what it is, never by its content, so a new version that says what an earlier one said is given; a latest version the session holds is given by `held`'s line, though the session lacks the version the cell last worked from.
Why: Holding by content swallowed what was new: the owner saying the same twice, or a datum changed back, would not be given, while the cell was told of a version it never saw. Giving the latest whole where the version last worked from is lacking was turned down: the change since it is unseen either way, and the session holds every word.

### decision the-situations-written-stand-data-by-class
`wf-agents flow situations` writes each situation an entry of a cell can reach with the data in two classes, each standing alike where it can, those new to the cell and the rest, and counts every combination of each datum's own state without writing it.
Why: The builder words a datum from its kind and how the session stands with it alone, so the product of every datum's states repeats the same texts, and runs to millions for a cell given ten data (`T1`, about 35 billion); by class every state of every datum is written, in a few hundred files a cell.

### decision the-situations-follow-what-the-flow-can-reach
The situations written are those the flow's definition lets a cell reach: a cell first on its line is entered first holding nothing, another after the cell of its line before it; what only the cell or cells after it write is unwritten on its first entry; what the work starts with, and what earlier cells write whichever way the conditions go, is never unwritten; a reopened session holds nothing; results and a return sent back come after a return that handed nothing in.
Why: Situations the flow cannot reach gave the models that read them findings that were not faults. A cell that may write a block is taken to write it, since the flow says no more than what a cell must write; where that is too much, `must_write` on the cell says what it always writes.

### decision a-version-is-told-by-the-line-that-wrote-it
Each version a round is given says the line of the cell whose run wrote it, or that the owner wrote it, worked out
where it is given from the writing run the records keep and the cell's line in the flow, and never stored. The owner's
words to a cell are the owner's. A version of a cell's questions is told by its own record: an ask by the asking run's
line, an answer as the owner's, the engine's own question by nobody; any other version the outside or an act of the
engine wrote says nobody, one put into a round as main is taken in among them (decision
a-version-read-at-a-rounds-end-is-that-rounds). The line is the
cell's in the flow the engine reads now, the records keeping no line of a run: a run of a cell since moved to another
line is told by the line it has now.
Why: A stored label would be a second copy of what the records and the flow say (principle one-thing-one-place), and a
line, not a cell, is what a round may know (principle a-cell-knows-only-its-data). Both sides of `plan_doubts` are on
one line, so their versions share a label, which is enough: the session holds them, the content tells a doubt from an
answer, and who owes the next word is read by newness.
