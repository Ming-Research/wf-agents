# The walk

What decides, from a work's records and the flow, which cell runs next.

### decision conditions-and-preconditions
A cell whose condition is not met is skipped, and what it wrote before still counts: the blocks it wrote stay as they
are, and nothing but a newer write changes them; only a test of what stands leaves them out. A gate is no cell but a
precondition of the cell it guards, worked out at every walk and never stored.
Why: A cell skipped this time has not been undone: a review that finds the tests enough does not make the tests
written before worthless. A gate drawn as a cell needed a record of what the work was waiting for, and care to raise
and withdraw that record at the right moments.

### decision a-way-back-waits-for-the-pass
The walk tells a way back from what is new to a cell and who wrote it, and keeps no record of passes; a cell with
preconditions ends the look past a way back, and is reached only by going through the flow from its start (design
document `data-and-context`, 6).
Why: Taken first, each judge's point went back to the plan's writer before the next judge saw the plan: in W11 the
research, the better-way cell and the design review each ran four times, and the review against the tree first ran at
the twelfth run, its one point running the three again. A batch recorded as it begins was turned down: what decides the
next cell would be more than the records' newness (principle state-from-records), and a cell a batch-mate makes new
after it would wait a batch for nothing, as the adversarial testing would after the test review asked for it. Looking
past a gate would take a verdict while its judge has something new, as the check before the merge would take `plan_ok`
on an acceptance the implementation changed.

### decision a-test-of-content-reads-the-record
A named test may hold where a block it reads holds anything (`any_content`): the record of each write says whether the
version holds nothing, no value and its content empty or only whitespace, and the walk reads that as it reads a
choice's value.
Why: A choice beside the block, written yes where the block held an entry, copied by hand what the engine sees itself,
and its writer could let the two disagree. Giving the walk the contents beside the records was turned down: what
decides a work would then be more than its records.

### decision every-way-round-has-a-limit
Each thing the flow lets repeat has a limit: how many runs of a cell in a row are ways round, how often what a run
returns is sent back, and how many runs of a cell in a row each ask for calls and deal with nothing before it is entered
again for their results; at a limit the work waits for the owner. A run is a way round when every version new to it as
it started was written by a cell after it, and its return dealt with something. A run with a reason that a person, the
outside or a cell before it wrote starts the count anew, and so does the owner's letting a cell go on; a run that dealt
with nothing, or was entered only for the results of its calls, is passed over. The count of calls is ended by a run
that dealt with something.
Why: In any circle of cells that send each other round, the first of them in the order runs only for what the later
ones wrote, so its count grows whatever the others' reasons are, and one count bounds them all. A person's opinion is
the owner steering and no circle, so it starts the count anew, and the owner is not asked to let go on what the owner
sent back (principle no-unseen-spending-loop). A run that dealt with nothing leaves what ran it new and is entered again
at once, so it is no step round the circle; a return that deals with nothing and asks for nothing is sent back, and
one that only asks is bounded by the count of calls, so that neither repeats unseen.

### decision meaning-told-by-the-writer
A block that is a choice or a list of names changes its meaning when its value does, whoever writes it, its first
version included, and a return that hands a choice in as changing nothing in meaning with another value is sent back.
Prose changes its meaning when its writer says so: with each write the cell says whether the change matters to the cells
the block runs, compared with the version at which the meaning last changed, and a first version said not to matter,
whoever wrote it, has no meaning until a version that matters. Prose a person writes always changes the meaning, and so
does what the outside writes, unless whoever approved that write said it does not. A return that would write over a
version of a block its round was never told of is sent back, the next round told of it, so a cell's word that a change
does not matter is always said of what it saw; an entry added to a dialogue whose versions all stand, a cell's questions
or its words, writes over nothing. What a change that does not matter leaves undone is in the design of
data and context.
Why: An agent never writes the same text twice, so judging prose by its text would run every later cell again after
every rerun. A second agent to check the answer, at a round for every change, was turned down, and so was a count that
goes up at each change of meaning, which would leave which version a write is compared with to whoever counts (Article
3). A run abandoned for what changed under it, as the walk once did, threw away a round's work that a round told of the
change could keep (principle keep-what-was-paid-for). A run's first version said not to matter is taken at its word as
any other is, though there is nothing it leaves unchanged: giving it a meaning all the same, or sending its return back,
was turned down as a case apart for one kind of version (principle solve-within-the-design-first); an agent that so
marks a first version leaves the cells the block runs unrun, which shows where the work stalls.

### decision a-question-that-waits-holds-its-cell
A question a cell waits for holds the work at that cell while it stands with no answer to its latest version, whether or not the cell applies now: no later cell runs, and the cell is entered meanwhile only for the owner turning to it, by a version no run of it has seen or written, and for the results of the calls such a run asked for, its round held to nothing its cell must write, as the cell stood when the round began; a run of it that is at rest still goes on to hand its return in. The answer runs the cell again by a version of its questions; the wait is read from the questions and the answers, and is no call.
Why: Put to the owner as a call, the question was its run's and not the work's: it could not be asked anew or withdrawn
under its name, an answer to an older version could not be told from one to the latest, and an answer came back twice,
as the call's result and as a version of a block of an answer (design document `data-and-context`, 30, 32). Entering the cell
meanwhile for something else new to it was turned down: that round could settle nothing the answer may not undo, and
what is new stays new for the round the answer begins (a judgment, with no case behind it). The owner's words are let
in, since the cell answers them at once and goes on waiting: words about the question a cell waited on reached no
round until the answer (work 4); so are the results a round answering them asked for, which otherwise waited for the
answer too. A round so entered whose question is answered while it runs owes nothing it was not told it owed: judged
as the work stands when it returns, it was sent back for not writing what the answer, not the owner's words, asks.
Passed while its cell was skipped, as the walk once did, a question of the research that waited for the owner let the
check before the merge run, the exploration of the engineer's flow found; the owner's answer is given before anything
after the cell runs, whether or not the cell applies when it comes.

### decision the-owner-turning-to-a-waiting-cell-enters-it-once
While a cell waits for the results of its calls, the owner turning to it, by words or by an answer, enters it only for
a version no run of the cell has seen: none that finished was told it, and none still open holds it. A run so entered
that asks for calls in turn leaves what the owner wrote new, for the run their results begin, which deals with both; a
run abandoned, its round stopped or its return unread, saw nothing, and the cell is entered for the version again.
Why: Entered again for the same version each time such a run asked, the cell went round without end, a call asked at
each turn, and no limit counted it, since the limit of calls counts only the runs entered for their results: a build
entered for an answer that asked for a tool first did so in the real carriers' tests. Counted from the cell's last run
begun instead, a version told to a run that was then abandoned waited for every result before any run answered it.

### decision a-work-no-cell-can-move-is-stalled
A work that waits only for what cells write, at a cell whose preconditions only cells make hold or at its last cell
that nothing new runs, is stalled, unless what those cells wrote waits for the outside (decision
what-a-run-wrote-may-wait-for-the-outside): the walk says where it stands, what it waits for, and why each cell that writes it
does not run, skipped, with nothing new, or held behind where the work stands. It is shown halted, as a fault is, with
that reason, worked out from the walk each time it is shown; it waits on no one, and the planner is given it as a line
of its own. Nothing is recorded for it: whatever then runs a cell moves it on.
Why: A wait that names only cells that will not run names nobody who can act (principle decide-purely): an
implementation that handed in nothing for a red check left the work at the merge's gate, waiting for the check, and
nobody was told. A pause of the engine's was turned down: its reason went stale once the work could move again, it
held what the owner wrote until they let the work on, and what it guarded against, the check run by main moving on a
branch no one reviewed, the check's own gate on the reviews' yes holds (decision the-check-waits-for-the-reviews).

### decision what-a-run-wrote-may-wait-for-the-outside
A finished run may say that what it wrote waits for a new version of a block a source outside the work writes and its
cell runs on (`waits_for`). Where a precondition is kept from holding only by blocks cells write, and the latest run of
each such cell says so and nothing of that block is new to it, the work waits for that source, with the run and the
block it wrote, and not for the owner; a new version runs the cell again. Only the engine's check says it, where what
fails also fails on main.
Why: Such a work was stalled, and so counted, marked and notified as waiting for the owner, who can do nothing for it
until main moves. Read by the page from the check's words, the wait would be a guess the engine does not make; taken as
a wait for the outside wherever the cell that would move the work is run by an outside block, a check that failed and
was handed nothing to answer it, which does need the owner, would be shown as waiting for main too.
