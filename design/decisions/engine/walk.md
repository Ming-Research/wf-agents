# The walk

What decides, from a work's records and the flow, which cell runs next.

### decision conditions-and-preconditions
A cell whose condition is not met is skipped, and what it wrote before still counts: the blocks it wrote stay as they
are, and nothing but a newer write changes them; only a test of what stands leaves them out. A gate is no cell but a
precondition of the cell it guards, worked out at every walk and never stored.
Why: A cell skipped this time has not been undone: a review that finds the tests enough does not make the tests
written before worthless. A gate drawn as a cell needed a record of what the work was waiting for, and care to raise
and withdraw that record at the right moments.

### decision a-test-of-content-reads-the-record
A named test may hold where a block it reads holds anything (`any_content`): the record of each write says whether the
version holds nothing, no value and its content empty or only whitespace, and the walk reads that as it reads a
choice's value.
Why: A choice beside the block, written yes where the block held an entry, copied by hand what the engine sees itself,
and its writer could let the two disagree. Giving the walk the contents beside the records was turned down: what
decides a work would then be more than its records.

### decision the-same-sender-again
A cell sent back is counted by who sent it: the engine, for each of its returns the engine sent back; a cell after it,
for each run of it that dealt with a version that cell wrote; the result of what the cell asked for comes from no one.
One sender may send the cell back as many times in a row as the flow's `again` says, and at the next the work waits for
the owner at the cell, naming the sender. A run with a reason that a person, the outside or a cell before it wrote
starts every count of the cell anew, and so does the owner's letting the cell go on, as the owner's word at a run's
limit does; a run that dealt with nothing is passed over.
Why: One count of runs that were ways round, and one of returns within a run, stopped courses that went well: work 9's
implementation, sent back by its testing, its review and the check in turn, each for a cause of its own, waited for the
owner at the third though no reviewer had repeated itself. A sender asking the same again is what runs away, and in a
circle the first cell in the order counts what each later one wrote, so a count for each sender still bounds every
circle. A person's opinion is the owner steering and no circle, so it starts the counts anew, and the owner is not asked
to let go on what the owner sent back (principle no-unseen-spending-loop). A run that dealt with nothing leaves what ran
it new and is entered again at once, so it is no step round. Counting runs that only ask among them was turned down:
asking is no sending back, and how often a cell asks is judged by its history (decision
asking-is-judged-by-its-history).

### decision meaning-told-by-the-writer
A block that is a choice or a list of names changes its meaning when its value does, whoever writes it, and a return
that hands a choice in as changing nothing in meaning with another value is sent back. Prose changes its meaning when
its writer says so: with each write the cell says whether the change matters to the cells the block runs, compared
with the version at which the meaning last changed. Prose a person writes always changes the meaning, and so does what
the outside writes, unless whoever approved that write said it does not. A return that would write over a version of
a block its round was never told of is sent back, the next round told of it, so a cell's word that a change does not
matter is always said of what it saw; an entry added to a record whose versions all stand, which a trigger of its
source has its agent read, writes over nothing. What a change that does not matter leaves undone is in the design of data and
context.
Why: An agent never writes the same text twice, so judging prose by its text would run every later cell again after
every rerun. A second agent to check the answer, at a round for every change, was turned down, and so was a count that
goes up at each change of meaning, which would leave which version a write is compared with to whoever counts
(Article 3). A run abandoned for what changed under it, as the walk once did, threw away a round's work that a round
told of the change could keep (principle keep-what-was-paid-for).

### decision a-wait-holds-its-cell
A cell waits while what it asked for has not come: the result of a call of a tool; the result of the tests it asked for
by its latest entry of `run_tests`; the owner's answer to the latest version of a question it waits for. A waiting cell
holds the work: no later cell runs but what ends the wait, the engine making the call, the cell the record runs giving
the result, the owner answering; and the cell is entered meanwhile only for the owner turning to it, by a version no
run of it has seen, and for the results of the calls such a run asked for, as the cell stood when the round began; a
run of it that is at rest still goes on to hand its return in. A return that leaves its cell waiting owes nothing else:
it may leave out the blocks its cell must write every time and deal with nothing. A result comes in a block of the
cell's own, which the cell that gives it hands in with the end of its round, as one of its outputs, and which runs the
asking cell alone and every run of it told of it takes; it is the end of a wait, named in no `dealt_with`. An entry
replaced by a newer one of the same cell has no result, and what it asked the host for is cancelled. The answer runs
the cell again by its trigger of an answer; a wait is read from the records, and a question is no call.
Why: Each wait was an exception of its own, the question that waits, the round entered for the owner while its cell
waited for calls, and those planned for a test run, and each left holes: a test run's commit moved the work's head, and
the review ran on it while the implementation waited for the result; an implementation given a green result was sent
back for not dealing with it, though it had nothing to do but hand its work in. One rule for every wait closes them.
Put to the owner as a call, a question was its run's and not the work's: it could not be asked anew or withdrawn under
its name, an answer to an older version could not be told from one to the latest, and an answer came back twice, as the
call's result and as the trigger of an answer (design document `data-and-context`, 35, 36). Entering the cell meanwhile
for something else new to it was turned down: that round could settle nothing the result may not undo, and what is new
stays new for the round the result begins (a judgment, with no case behind it). The owner's words are let in, since the
cell answers them at once and goes on waiting: words about the question a cell waited on reached no round until the
answer (work 4); so are the results a round answering them asked for, which otherwise waited for the answer too. A
round so entered whose question is answered while it runs owes nothing it was not told it owed: judged as the work
stands when it returns, it was sent back for not writing what the answer, not the owner's words, asks. A result named
as dealt with would be held to the rule that an act's block is answered (decision an-act-is-answered-by-what-runs-it),
and a green result would send the cell back for answering it by nothing but its work. Written from outside in the
middle of the round that gave it, a result ended the wait before that round ended, and the owner's word to a cell the
wait had held behind it stopped the round, which lost what it ran from its history and ran again for nothing; handed in
with the round's end, it is what the round did, on its return and on the chart.

### decision a-wait-of-a-skipped-cell-is-put-away
A wait of a cell that no longer applies is put away: it holds nothing, the cell that gives what it waits for gives it
nothing, and it is waited for again once the cell applies again. The cell that gives the result of an entry is then
entered for it, though it took the record in before, passing over the entry while its cell was skipped. Of a question,
the engine's record of the questions' standing (`asked`) holds the merge whatever cell asked it, skipped or not.
Why: Run for a cell that no longer applies, its entry ran later on another cell's commit, for a cell whose work no
longer counted, and gave a result to none that read it. One rule for every wait keeps a skipped cell from holding, or
spending, for what it no longer does.

### decision the-owner-turning-to-a-waiting-cell-enters-it-once
While a cell waits (decision a-wait-holds-its-cell), the owner turning to it, by words or by an answer, enters it only
for a version no run of the cell has seen: none that finished was told it, and none still open holds it; and only such a
version, of a cell no waiting cell holds back, stops a round under way of a later cell. A run so entered that asks in
turn leaves what the owner wrote new, for the run the results begin, which deals with both; a run abandoned, its round
stopped or its return unread, saw nothing, and the cell is entered for the version again.
Why: Entered again for the same version each time such a run asked, the cell went round without end, a call asked at
each turn, and no limit counted it: a build entered for an answer that asked for a tool first did so in the real
carriers' tests. Counting a version the cell was told as the owner turning to it stopped a round for words the asking
cell had answered before it asked. And words to a cell held behind a wait would stop the very round that is ending that
wait, while the cell spoken to can run only once the wait is over: the work would be held for ever. Counted from the
cell's last run begun instead, a version told to a run that was then abandoned waited for every result before any run
answered it.

### decision a-work-no-cell-can-move-is-stalled
A work that waits only for what cells write, at a cell whose preconditions only cells make hold, at a cell that waits
for the result of what it asked for in a record, which only the cells its record runs give and no person or source
outside the work can make run, or at its last cell that nothing new runs, is stalled, unless what those cells wrote
waits for the outside (decision what-a-run-wrote-may-wait-for-the-outside): the walk says where it stands, what it waits
for, the keys or the results, and why each cell that writes it does not run, skipped, with nothing new, or held behind
where the work stands. It is shown halted, as a fault is, with
that reason, worked out from the walk each time it is shown; it waits on no one, and the planner is given it as a line
of its own. Nothing is recorded for it: whatever then runs a cell moves it on.
Why: A wait that names only cells that will not run names nobody who can act (principle decide-purely): an
implementation that handed in nothing for a red check left the work at the merge's gate, waiting for the check, and
nobody was told; and a wait for the result of an entry stayed a wait whatever its giver could do, so a giver whose
condition nothing would make hold left the work waiting for good, nobody told. A pause of the engine's was turned down: its reason went stale once the work could move again, it
held what the owner wrote until they let the work on, and what it guarded against, the check run by main moving on a
branch no one reviewed, the check's own gate on the review's yes holds (decision the-check-waits-for-the-review).

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
