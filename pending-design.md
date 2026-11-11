# Designs ahead of the code

Decisions that were designed but are not yet true of the code on main. They are kept out of the design tree, which
holds only what the code does; each becomes an item, or is dropped, and enters the tree with the change that makes it
true. `Gap` says what the code does instead, where there is code.

## Flow

The words these entries use, each with one meaning:

- A work's **state** is its **blocks**: the named parts of its item, which cells, people and the outside write and
  cells read. A **version** of a block is one write of it, and its **current** version is the latest.
- A version's **meaning** is the version at which what the block says last changed for those who read it: the version
  itself, unless it changes nothing of what the block says (decision meaning-told-by-the-writer), when it is the
  meaning of the version before it.
- A cell's **inputs** are the blocks it reads, and its **outputs** the blocks it may write.
- A **run** is one execution of a cell for a work. It is made of **rounds**: in a round an agent is given something and
  returns, or the engine does an act of its own. Between rounds a run is **at rest**. A run **starts from** the current version
  of each block its cell reads or may write. A run **finishes** when its return has met everything the flow requires of
  it, and writes its outputs then and not before; a run is
  **abandoned** when the engine gives it up before that. A cell's **record** is its latest finished run. A record
  **knows**, of each input, the version its run started from, or the version it wrote itself.
- An **opinion** is what someone has to say to a cell about what it did. A cell that can be told something has a block
  of opinions among its inputs.
- A cell is **skipped** when its condition is not met.
- A run at rest is **stale** when its cell is skipped, or when a block it started from has changed in meaning.
- A run is a **way round** when each of its reasons is a block whose meaning a cell after it last changed.
- A work's **records** are all that is recorded of it: its runs, their rounds and calls, and every write of every
  block. They are only added to.
- The **walk** is the engine going through the cells in their order to find what a work does next.
- A **call** is something a run rests at until it has its result: a tool the engine runs, a ruling or a start a person
  gives. A **line** is the cells of a work that share one session. The **merge** is the flow's last cell.

### decision state-is-blocks
A work's state is the blocks of its item, and a cell is joined to the other cells only through them: it reads its inputs, and what it hands in goes into the item as its outputs. A cell runs again only when one of its inputs has changed in meaning since it last ran: the same inputs give the same outputs, so a cell whose record knows the current meaning of every input is passed. Whoever wrote the change (a cell before it or after it, a person, the outside), the cell is run for it in the same way. A cell does not run for its own write.
Why: One rule then covers going on and going back: a first run, a rerun because something changed, a fix because someone objected, a second look because the fix changed what was judged. Cells wired output to input had to be rewired for every cell added; a way back written by hand had to say where to, with what, and what of the earlier work still counted; and a "no" kept as a thing of its own needed rules for who answers it, when it ends and whether it still has force, each of which brought cases the next review found. (principle state-from-records; Article 4)
From: the owner, 2026-11-09

### decision opinions-are-inputs
What someone has to say to a cell about what it did is written into that cell's block of opinions, which is one of its inputs: a review's findings for the implementation, the owner's objection to a plan, a remark the owner writes for that cell on the page. A new opinion is a new version of the block, so the cell runs again, is given what was said, and replies. What it answers to an opinion is a block the judging cell reads, so that an answer which changes nothing else still brings the judge to look again. The flow has no cell that takes the owner's remarks: the owner knows whether a remark is about the design, the implementation or the tests, and writes it for that cell. A cell that judges writes an opinion only when it has one: a review that finds nothing more writes nothing, and the cell it judged is not run again. The flow has no cell for fixing and none for checking a fix: what was objected to is fixed by its cell running again, and the fix is looked at by the judging cell running again because what it judged changed. The answer to a question of a cell is written where the question came from: an answer other than the one recommended, or an objection, as an opinion for that cell; a confirmation changes no meaning and only meets the precondition that waits for it. Whether a judging cell is content is a block it writes, and the merge waits on it.
Why: Going back is then the same thing as going on: an input changed. A fix is the work of the first writing done as an increment, and a second look the work of the first on what changed; cells of their own for the two needed their own rules for who fixes what a red CI before the merge found and who checks that fix. An opinion emptied when it is settled would be one more change to run a cell for; only a new one is. A cell that took every remark and handed it on would be a round to do what the owner does by choosing where to write, and what it handed on to a cell before it would count as that cell being sent round. With one way for a remark to change a work's course, an approval on a question and a "no" said elsewhere cannot remain side by side unresolved. (principle reviews-can-be-disputed)
From: the owner, 2026-11-09

### decision work-has-no-position
A work has no stored position. Each time a round ends, what it handed in having been taken, or anything else happens to a work, the engine walks the flow's cells from the first, in their order: it passes a cell that is skipped; at the first cell that has a run at rest, the run has its next round, unless the work must wait there; at the first whose preconditions are not met, or whose next run would pass its limit, the work waits, and says everything it waits for there at once; it runs the first that needs to run, and passes the rest. A precondition holds the work before its cell whether or not the cell needs to run. Before the walk, a run at rest that is stale is abandoned. While a round is under way nothing is decided, unless the work is cancelled. A work has ended when the merge has a record or its item is cancelled, and nothing more is decided for a work that has ended, once the runs of a cancelled one are abandoned; the merge has no condition. What is stored is the records; where the work goes next is walked.
Why: A stored position has to be read rightly by every rule. Two defects of an engine that stored a position were both there: a work sent back without end once its cell asked for a tool, and a way back judged by the number of each cell's first round. Walking from the first cell also takes first whatever else changed meanwhile: when the owner changes an answer after the plan was implemented, the plan is redone before anything built on it runs again. (principles state-from-records, decide-purely, report-all-at-once)
Not: Hierarchical state machines, which end the repeated rows but keep a position: a person's input stays an interruption to be hung on every state, and a changed flow still needs a place to land each work.
From: the owner, 2026-11-09

### decision inputs-and-outputs-declared
Each cell declares its inputs and its outputs. What a cell reads without declaring it never makes it need to run: an agent may read anything of the item, and only its inputs are what it is run again for. Main is an input only of the cell right before the merge, which checks that the work passes the full CI with the latest main taken in and writes an opinion for the implementation when it does not; a cell that changes the code takes main in as its run starts without reading it (decision a-run-that-changes-the-code-takes-main-in). The merge only merges.
Why: Whether a cell must run again is worked out from what it last ran on, by one rule. The check is a cell of its own because a work ends when the merge has a record: nothing the merge wrote could send the work back.
Not: Every cell reading main, which would run a whole work again at each merge of another.
From: the owner, 2026-11-09

### decision meaning-told-by-the-writer
A block that is a choice or a list of names changes its meaning when its value does, whoever writes it: the owner confirming what was recommended changes none. Prose changes its meaning when its writer says so: with each write the cell says, with a line of why, whether the change matters to the cells that read the block, compared with the version at which the meaning last changed, which it is given. Prose written by a person always changes the meaning, and so does what the outside writes, unless whoever approved that write said it does not, as the owner may of a cell's changed specification. A cell that says a change does not matter without having seen what it wrote over, because the block's meaning was changed after its run started, is not taken at its word. The answer and its reason are kept where the owner sees them, and the page marks where what the owner was last shown differs from the current version.
Why: An agent never writes the same text twice, so judging prose by its text would run every later cell again after every rerun: when an item's title is reworded and the research adjusts a few words of its plan, nothing after it should run. Comparing with the version at which the meaning last changed keeps a row of small changes, each said not to matter, from adding up unseen. The owner reads the designs where this arises, and the page's mark shows exactly the cases where a change was said not to matter. (Article 3)
Not: A second agent to check the answer, at a round for every change. Nor a count that goes up at each change of meaning: which version a write is compared with would then be left to whoever does the counting.
From: the owner, 2026-11-09

### decision conditions-and-preconditions
A cell may have a condition and may have preconditions. A condition says whether the cell runs this time: where an agent's choice decides it, by the value of a block that a cell before it, or a person, writes as a yes or a no (principle yes-no-forks); otherwise by a named test of the state. A cell whose condition is not met is skipped, and what it wrote before still counts: the blocks it wrote stay as they are, and nothing but a newer write changes them. A precondition says when the work must wait before the cell; it is a named test of the state, worked out at every walk and never stored. A gate is no cell but a precondition of the cell it guards, and what a work waits for, and who can give it, is the precondition not met. A named test reads the state and what people and the outside did, never whether a cell is running, at rest or finished.
Why: A fork drawn as rows and a gate drawn as a cell each needed a record of what the work was waiting for, and care to raise and withdraw that record at the right moments. A cell skipped this time has not been undone: a review that finds the tests enough does not make the tests written before worthless, and a research that sees no need to design again leaves the design as it is. A test that read the runs could make a cell's being on the way depend on its own running. (principle state-from-records)
From: the owner, 2026-11-09

### decision calls-inside-a-run
What a cell asks for while it runs (a tool, a pause, a ruling from whoever can decide when it cannot go on), and what the flow requires its return to meet (the engine's checks, a review that only advises), are calls of that run: the run rests at the call, the result comes back into its next round, and the run finishes when its return has met them. A call holds no place in the order of cells and writes no block; the flow declares it on the cell, and the chart draws it beside the cell. A call the flow requires is made by the engine, never left to the agent to ask for. The owner's letting a run go on at a limit is a call of that run too, whose result is only that the run goes on.
Why: Drawn as cells, three kinds of call took some thirty-seven of an engineer graph's ninety-four rows, the same three on every agent cell, and the engine kept a stack of who had called whom; where a work was then had two answers, the tool's cell and the cell the tool ran for. What a task only asks of an agent, the agent may leave undone, so a required call is the flow's.
From: the owner, 2026-11-09

### decision agent-reads-what-it-needs
Besides what every run of its line is given, a run is given its task; a list of its inputs, each with its current version and the version the cell last worked from, those that changed marked with who wrote the change; and, for each block it may write, the version at which its meaning last changed. The content of the item, and what differs between two versions of a block, it reads itself, the principles before the decisions, through the engine's tool for reading, which is a call of the run; the engine records what it read. What a run hands in comes back in its return. The engine keeps no account of what a session already holds.
Why: Only the agent knows what is in its context: a session that goes on remembers the old and reads what changed, a new one reads all. Deciding for it what to send again was a mechanism of its own. Run again for an increment, a cell must be able to see both what it worked from and what is there now. The engine's tool is the way in, and files in the worktree are not, because the engine and the machine an agent runs on may be apart, and reading the database goes through one entry the engine controls. Each read costs a round, so the list must let an agent ask for all it needs at once, and what the reads cost is measured. (principles lean-agent-input, measure-speed-and-cost)
From: the owner, 2026-11-09

### decision session-is-a-cache
A session, what a worktree holds uncommitted and the prompt cache are caches: losing one costs money and changes no result. What a later cell needs of an earlier one is in a block. The exception is a cell whose whole job is to load context: its output is the session as it was when the cell finished, kept in the session, and later cells of its line go on in it or fork it. That a session is alive is a block the outside writes and that cell reads: when the engine finds the session lost it writes the block, and the cell runs again; rebuilt from the same versions of what it read and the same specification, the output is written as a change that does not matter, so nothing after it runs again.
Why: A new session is then the same kind of event as a changed answer, and needs no rule of its own. A cell whose job was other work, run again only to restore what it knew, would find none of its inputs changed and hand in what it handed in before, reading nothing again; so only a cell whose task is the reading has such an output. (principle keep-what-was-paid-for)
From: the owner, 2026-11-09

### decision every-way-round-has-a-limit
Each thing the flow lets repeat has a limit: how many runs of a cell in a row are ways round, how often what a run returns is sent back, how many calls a run makes. Within a run, what a round returns is sent back when it does not meet what the flow requires, or when it asks for calls and names none; at the limit of returns sent back, or of calls, the run rests for the owner, who lets it go on or cancels the work. Let go on, the run does what its last round returned called for, the calls being made or the return sent back, and both counts start anew with the round after. A run is a way round when each of its reasons is a block whose meaning a cell after it last changed: an opinion on what it did, for one. Once a cell's last finished runs, as many in a row as its limit, were each a way round and the next would be one, the work waits for the owner, who lets the cell go on or cancels the work. The owner's letting a cell go on starts its count anew, whenever it is given. A run with a reason that a person, the outside or a cell before it wrote starts the count anew. The reasons of every run are recorded as it starts, and the counts are worked out from the records. The compiler refuses a flow in which something that repeats has no limit, or a limit of none.
Why: Something that repeats without a limit spends without end and unseen. In any circle of cells that send each other round, the first of them in the order runs only for what the later ones wrote, so its count grows whatever the others' reasons are, and one count bounds them all. A person's opinion is the owner steering and no circle, so it starts the count anew, and the owner is not asked to let go on what the owner sent back. (principle no-unseen-spending-loop)
From: the owner, 2026-11-09

### decision changed-flow-walks-the-records
A changed flow is installed by walking each work's records on the new graph: a cell that did not change is passed, a new cell has no record and runs when the walk reaches it, and what a cell taken out wrote stays in the item. The specification of a cell is an input of that cell: a change of it is a write from outside, and whether it matters to the works under way, so that they run the cell again, the owner says when approving the change. Walking without acting, before the change is approved, shows for each work from which cell it would run again. A record is found by its cell through the table of earlier ids (principle cell-numbering).
Why: Records are kept by cell and by the versions each run started from, not by a step in a course taken, so they still fit a graph that has changed; and a changed cell is one more input that changed. (principle flow-change-spares-works)
From: the owner, 2026-11-09

### decision stops-from-outside-are-no-cells
Cancelling is a fact on the item, pausing a switch on the work that lets no round begin, and how many rounds go on side by side is the scheduler's matter: none of the three is a cell or a block, and a work with no round under way holds no slot. Nothing of a cancelled work goes on: its round under way is stopped, the calls it rests at are withdrawn, and its runs are abandoned. A pause holds back only what would begin a round: while a work is paused what a round handed in is still taken, the calls it asked for are made, and a stale run is abandoned.
Why: What stops or holds a work from outside applies wherever the work is, which a cell cannot. A round of a cancelled work is paid for nothing, and a question of it left open would be put to someone again when the engine next starts. A pause that also held back the taking of what a round handed in would let the work change under it, and the round would be paid for a second time.
From: the owner, 2026-11-09

## Backend

### decision engine-knows-no-cell
The engine's code names no particular cell and no particular block: it carries out whatever the flow's definition holds. The definition holds each cell's specification (its task, the form of what it hands in, its checks, its line) and how it is joined (its place in the order, inputs, outputs, condition, preconditions, calls, limits). A named test, for a condition or a precondition, is a function in code; a new kind is a new function. The compiler checks that every block has a writer, a cell, a person or the outside, and a reader, a cell, a condition, a named test or the page; that a condition reads a choice which its own cell does not write and which a cell before it or a person writes; that everything that repeats has a limit; that every named test is one the engine has; and that the merge has no condition.
Why: What is in the definition the owner can read on the chart and the compiler can check before anything runs. (Article 7; principles compile-before-run, every-field-has-a-reader)
From: the owner, 2026-11-09

### decision one-loop-decides
One loop takes what happens, in order: it adds to the records, and walks each work that has not ended and has no round under way. A round under way is not interrupted, unless the work is cancelled: what arrives meanwhile is in the records when the round ends, and the walk taken then decides with it. The rounds of different works go on side by side. The deciding code holds no lock. Records that cannot be those of a work (a run that never started, two entries at one place, a block written twice by one run) are said so by the deciding code, which decides nothing on them.
Why: Decisions taken one after another are the same every time the same things happen in the same order, which is what lets a history be simulated and run again. (principles simulated-histories, nothing-passes-for-normal)
From: the owner, 2026-11-09

### decision a-run-is-one-mechanism
Running a cell is one mechanism, the same for every cell: take the session of the cell's line, starting one where there is none and forking the loaded context where the cell reads it; give the task; wait for the return; make the calls it asks for and the calls the flow requires; and, when its return has met what the flow requires, record in one entry the blocks it wrote, and for each whether the change matters. A run writes no block before it finishes. What a round hands in that meets everything the flow requires is taken as the round ends: the run finishes there, before the walk, so that no cell before it comes between; a run that is stale when its last round ends is abandoned, not finished. A precondition of a cell holds back the beginning of its run, not the finishing of one that has begun. A run interrupted by a stop of the engine goes on from what its rounds recorded. Each kind of call says how it starts, when it is complete, and by what it is recognised as already made.
Why: Written once, it is tested once, and a new kind of call touches no cell. A block half written by a run that is then abandoned would have to be told apart, at every reading, from one that counts; a run finished on what had changed under it would write over what it never saw; and a run left unfinished while a cell before it ran would be made stale by that cell, and what it had handed in would be paid for again. (principle effects-known-as-done)
From: the owner, 2026-11-09

### decision engine-holds-no-state
Everything kept of a work is its records and the content they name. The engine's loop holds nothing between two things that happen: it takes one, adds it to the records, asks the deciding code what follows until the work has a round under way, waits or has ended, and gives what is to be done outside as data: a round to give or to act out, a call to make or to withdraw, a round to stop. What was to be done and is not yet known as done is worked out from the records whenever it is asked for: of a run that goes on, its round under way and its calls with no result; of a run that was abandoned, the stopping of its round and the withdrawing of its calls, each until it is said to have been done.
Why: An engine that held the state of a work in memory would lose it at every stop, and what it rebuilt on starting could differ from what it had. Worked out from the records, what is under way is the same before a stop and after it, and a simulated history can stop the engine at any step and compare. (principles state-from-records, effects-known-as-done, simulated-histories)
From: the owner, 2026-11-09

### decision engine-in-layers
The engine's code is in layers, each depending only on those before it: the deciding code, which depends on the definitions alone; storage; the real implementations of the interfaces; the loop; the owner's interface.
Why: The order is the order of what can be tested alone: the deciding code with no storage, storage with no agent, the loop with test implementations of everything outside it. (principle dependencies-one-way)
From: the owner, 2026-11-09

### decision tables-only-added-to
The tables the deciding code is given are only added to, each row with its place in the order of what happened, and no two rows of a work at one place. There is no general store of events beside them.
Why: The deciding code is given a work's records as they were at a moment, and tables only added to give that with no copy kept. A general store of events would be a mechanism with no second use. (principles state-from-records, generalise-at-the-second-use)
From: the owner, 2026-11-09

## What the engine always keeps

The properties the simulated histories check after every step (principle simulated-histories).

### decision one-round-of-a-work-at-a-time
No work has two rounds under way at once. A run is running only while one of its rounds is under way: between rounds it is at rest, whatever it rests for (a person's ruling, a tool's result, or only its next round), and a cell before it may run meanwhile. A run at rest that is stale is abandoned, its calls withdrawn and no record left; where its cell is not skipped, the cell then runs again in the same session, with what changed among its reasons.
Why: The cells of a work share one worktree and one branch, and a round is the stretch in which an agent or the engine works on them; between rounds nothing does. A run may rest for days for a person, and a remark or a changed answer must be able to reach the work meanwhile: the walk takes what arrived in its order, the cell it was written for running first. A run that went on from what has since changed would be paid for twice, and the answer to its call would be given for nothing; a remark for another cell that changes nothing for it leaves the run, and the ruling it waits for, as they were. (principle keep-what-was-paid-for)
From: the owner, 2026-11-09

### decision no-merge-on-what-is-not-settled
No work merges while a precondition of the merge is not met, among them that every judging cell is content, and the merge is made only of the commit that the check before it passed.
Why: A merge cannot be taken back, and each of these is someone's "not yet". (principle nothing-passes-for-normal)
From: the owner, 2026-11-09

### decision merge-only-on-what-is-current
As the merge's last round starts, the record of every cell that is not skipped knows the current meaning of each of its inputs.
Why: A merge rests on every record before it, and one that knows a version no longer current carries a decision someone has since changed.
From: the owner, 2026-11-09

### decision a-work-comes-to-rest
When people do nothing more and nothing outside the work changes, every work, after a number of rounds its limits bound, waits, saying for what and for whom, or has ended.
Why: A work that neither waits nor ends is spending. (principle no-unseen-spending-loop)
From: the owner, 2026-11-09

### decision records-give-the-same-walk
Walking a work's records as they were at any moment, on the flow then installed and with the named tests as they then came out, gives the decision the engine took at that moment.
Why: The page's account of why a work is where it is, the tracing of a fault and the showing of what a changed flow would do all walk the records again. (principle decide-purely)
From: the owner, 2026-11-09

### decision a-stop-changes-nothing
For the same returns of the agents and the same acts of people, a history in which the engine was stopped at any step and started again ends with the same blocks and the same decisions, each effect made once, as the history without the stop.
Why: An engine that is stopped to be replaced, or that fails, must cost no work and repeat no effect. (principle effects-known-as-done)
From: the owner, 2026-11-09
