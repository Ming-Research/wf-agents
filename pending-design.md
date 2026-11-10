# Designs ahead of the code

Decisions that were designed but are not yet true of the code on main. They are kept out of the design tree, which
holds only what the code does; each becomes an item, or is dropped, and enters the tree with the change that makes it
true. `Gap` says what the code does instead, where there is code.

## Flow

The words these entries use, each with one meaning:

- A work's **records** are all that is recorded of it. They are only added to.
- A **run** is one execution of a cell for a work. It is made of **rounds**: in a round an agent is given something and
  returns, or the engine does an act of its own. Between rounds a run is **at rest**. A run **finishes** when its
  return has passed everything the flow requires of it; one that does not finish is **abandoned**. A cell's **record**
  is its latest finished run.
- A **product** is a thing with a name and versions that cells depend on: a block of the item, an output of a cell, or
  something outside the work, such as main. A **version** is one write of it. A run writes its products as it finishes
  and not before; people and the outside write at any time. A product's **current** version is the latest written by a
  person, from outside, or by a finished run of a cell on the path. A cell **sees**, of a product it depends on, only
  what a person, the outside or a cell before it wrote.
- A version's **meaning** is the version at which what the product says last changed for the cells that depend on it:
  the version itself, unless its writer said the change does not matter, when it is the meaning of the version the
  writer's run wrote over: the latest written by a person, from outside, by a cell before the writer or by the writer's
  own cell.
- A cell **depends on** the products it declares it depends on, and **may write** those it declares it writes.
- A cell is **on the path** of a work when its condition is met by the records as they are. A cell off the path counts
  as having no record: what it handed in is absent to every cell, and a refusal by it, or of a version it wrote, has no
  force.
- A **refusal** is a cell's or a person's "no" to a version of a product. It **stands** until the product is written
  by a person, from outside, or by a finished run that was given the refusal. It **has force** when the version it
  refused is not one a cell off the path wrote and, where a cell made it, while the run that made it is that cell's
  record. A refusal is **in force** when it stands and has force. A run is **given**, as it starts, every refusal in
  force against a product its cell may write. The cell that **answers** a refusal is the first cell on the path that
  may write the product and either has no record or wrote the product in its record. No refusal is ever taken out of
  the records.
- A run at rest is **stale** when its cell is no longer on the path, when something it started from, or
  is to write over, has changed in meaning, or when a refusal its cell answers is in force and was not given to it.
- A run is a **way round** when none of its reasons came from before its cell or from a person.
- The **walk** is the engine going through the cells in their order to find what a work does next.
- A **call** is something a run rests at until it has its result: a tool the engine runs, a ruling or a start a person
  gives. A **line** is the cells of a work that share one session. The **merge** is the flow's last cell.

### decision work-has-no-position
A work has no stored position. Each time a round ends, or anything else happens to a work, the engine walks the flow's cells from the first, in their order: it skips a cell that is not on the path; at the first cell that has a run at rest, the run has its next round, unless the work must wait there; at the first whose preconditions are not met, that depends on a product against which a refusal is in force that no cell answers, or whose next run would pass its limit, the work waits, and says everything it waits for there at once; it runs the first that needs to run, and passes the rest. A precondition holds the work before its cell whether or not the cell needs to run, and a refusal in force that no cell answers holds it before the merge whatever the product. Before the walk, a run at rest that is stale is abandoned. While a round is under way nothing is decided. A work has ended when the merge has a record or its item is cancelled, and an ended work is not walked; the merge has no condition. What is stored is the records, among them who refused what; where the work goes next is walked.
Why: A stored position has to be read rightly by every rule, and a way back written by hand has to say where to, with what, and what of the earlier work still counts. Two defects of an engine that stored a position were both there: a work sent back without end once its cell asked for a tool, and a way back judged by the number of each cell's first round. Walking from the first cell also takes first whatever else changed meanwhile: when the owner changes an answer after the plan was implemented, the plan is redone before anything built on it runs again. (principles state-from-records, decide-purely, report-all-at-once)
Not: Hierarchical state machines, which end the repeated rows but keep a position: a person's input stays an interruption to be hung on every state, and a changed flow still needs a place to land each work.
From: the owner, 2026-11-09

### decision cell-runs-when
A cell needs to run when it has no record, or when one of these is true of its record: a product it wrote was lost; something it depends on, as the cell sees it, has a meaning other than that of the version the record's run started from; a refusal it made no longer stands; or it is the cell that answers a refusal in force. A cell sees only what was written before it, so it never needs to run for its own write, nor for the write of a cell after it: the only way back is a refusal.
Why: These are all there is to going on and going back: a first run, a rerun because what it was built on changed or was lost, a rerun to judge again, a rerun because its product was judged wanting. Were a later cell's write of a block a change for the cells before it, a cell that judges a plan would run again each time a cell after it rewrote a part of the plan, and the two would send each other round to their limits though neither refused the other; and a circle closed through a condition, a later cell's answer taking off the path what an earlier one depends on, would never come to rest. Running again every cell after one that ran again was refused as the rule: a design redone need not mean an implementation redone when what the implementation depends on did not change. (Article 3)
From: the owner, 2026-11-09

### decision dependencies-declared
Each cell declares what its result depends on: an output of a cell before it, blocks of the item, or something outside the work such as main or the project's settings. Of a block that several cells write, a cell depends on what people, the outside and the cells before it wrote. What a cell reads without declaring it never makes it need to run. A dependency names a cell's output, not the branch as it is, where that output is what the cell judged. A work is not held to the main it started from: a cell that changes the code takes the latest main into the work's branch as its run starts, and main itself is depended on only by the cell right before the merge, which checks that the work passes the full CI with the latest main taken in and refuses the implementation's output when it does not. The merge only merges.
Why: Whether a cell must run again is then worked out from what it was built on, by one rule and one kind of dependency. A review that depends on what the implementation handed in is not started over by every commit after it. A work held to one main from its start to its merge meets what others merged meanwhile only in the merge queue, where a conflict costs the most; taking main in at every run that changes the code shows it while the work is still being done. The check is a cell of its own because a work ends when the merge has a record: a refusal by the merge could never have force.
Not: Every cell depending on main, which would run a whole work again at each merge of another.
From: the owner, 2026-11-09

### decision meaning-told-by-the-writer
An output that is a choice or a list of names changes its meaning when its value does, whoever writes it. Prose changes its meaning when its writer says so: with each write the cell says, with a line of why, whether the change matters to the cells that depend on the product, compared with the version at which the meaning last changed, which it is given. Prose written by a person, or from outside the work, always changes the meaning. A product written anew in answer to a refusal makes its refuser need to run, whatever its writer says; other cells go by the writer's word. The answer and its reason are kept where the owner sees them, and the page marks where what the owner was last shown differs from the current version.
Why: An agent never writes the same text twice, so judging prose by its text would run every later cell again after every rerun: when an item's title is reworded and the research adjusts a few words of its plan, nothing after it should run. Comparing with the version at which the meaning last changed keeps a row of small changes, each said not to matter, from adding up unseen. An answer to a refusal must reach the one who refused. The owner reads the designs where this arises, and the page's mark shows exactly the cases where a change was said not to matter. (Article 3)
Not: A second agent to check the answer, at a round for every change. Nor a count that goes up at each change of meaning: which version a write is compared with would then be left to whoever does the counting, and a write said not to matter after a version that is no longer current would hide a real change.
From: the owner, 2026-11-09

### decision refusal-runs-the-writer-again
While a refusal is in force, the cell that answers it needs to run, and every run of a cell that may write the product is given who refused and why. What a run given the refusal writes of the product ends the refusal and brings the refuser to judge again; the cells that depend on the product run only if the write changed its meaning. A cell that, given the refusal, does not write the product again leaves it to the next cell that may write it, which then answers. A refusal in force that no cell answers holds the work for the owner, who writes the product anew or cancels the work. A refusal made while a run of the cell that answers it goes on makes that run stale once its round has ended: the cell runs again in the same session, given the refusal. Grounds for disagreeing are a write too, and make the refuser judge again; grounds the refuser does not accept go, as a call of the writer's next run, to whoever the flow names to decide between them.
Why: A refusal says only what is wrong, and the walk finds where the work goes: no way back says where to. A run that was not given a refusal cannot have answered it, so only the write of a run given it ends it; a run left to finish a product already refused would be paid for a second time. The first cell that may write the product answers because what the later ones wrote was built on its version; one that hands the product on, as research does when it sends a refused plan to design, is not asked again for what it no longer writes. A refusal nobody answers, passed over, would let the work merge against it. (principles reviews-can-be-disputed, nothing-passes-for-normal)
From: the owner, 2026-11-09

### decision fixing-is-running-again
The flow has no cell for fixing and none for checking a fix. What was refused is fixed by the cell that wrote it running again, from what is current and given the refusal; the fix is checked by the cell that refused running again, on what changed since it judged. A cell's task says how it works when it runs again.
Why: A fix is the work of the first writing done as an increment, and a second check the work of the first on what changed. Cells of their own for the two needed their own rules for who fixes what a red CI before the merge found and who checks that fix, and a graph that could only go forward needed them to say where to go back to. The check before the merge refuses the implementation's output as any reviewer does, and the walk does the rest.
From: the owner, 2026-11-09

### decision conditions-and-preconditions
A cell, or a stretch of consecutive cells, may have a condition; a cell may have preconditions. A condition says when it is on the path: where an agent's choice decides it, by the yes/no answer of the cell right before (principle yes-no-forks); otherwise by a named test of what cells before it, people or the outside wrote. The compiler refuses a condition that reads the cell's own output or a later cell's. A precondition says when the work must wait before the cell; it is a named test of the records as they are, worked out at every walk and never stored beside them. A gate is no cell but a precondition of the cell it guards, and what a work waits for, and who can give it, is the precondition not met.
Why: A fork drawn as rows and a gate drawn as a cell each needed a record of what the work was waiting for, and care to raise and withdraw that record at the right moments. A test of the records as they are cannot be left behind after its cause is gone. A condition that read what its own cell or a later one handed in would decide whether a cell is on the path by what only being on the path produces. (principle state-from-records)
From: the owner, 2026-11-09

### decision calls-inside-a-run
What a cell asks for while it runs (a tool, a pause, a ruling from whoever can decide when it cannot go on), and what the flow requires its return to pass (the engine's checks, a review that only advises), are calls of that run: the run rests at the call, the result comes back into its next round, and the run finishes when its return has passed. A call holds no place in the order of cells; the flow declares it on the cell, and the chart draws it beside the cell. A call the flow requires is made by the engine, never left to the agent to ask for.
Why: Drawn as cells, three kinds of call took some thirty-seven of an engineer graph's ninety-four rows, the same three on every agent cell, and the engine kept a stack of who had called whom; where a work was then had two answers, the tool's cell and the cell the tool ran for. A review that advises once, and whose remarks return to the cell that asked, is the same thing as a tool. What a task only asks of an agent, the agent may leave undone, so a required call is the flow's.
From: the owner, 2026-11-09

### decision a-person-judges-outside-the-order
The owner's confirmation of the answers to an item's questions is a judgment made at any moment, outside the order of cells. An answer other than the one recommended, or an objection, refuses the question's current version. A confirmation is of the question's current meaning, and lasts while the question keeps that meaning.
Why: A person cannot be scheduled, so the judgment is written into the records when it arrives, under the rule for any refusal. Refusing the question, not one writer, leaves no choice of where to send the work.
From: the owner, 2026-11-09

### decision comments-through-one-cell
A comment never steers a work by itself. One cell, placed before the implementation, takes the comments: it is on the path once the item has a comment, which is a test of what a person wrote, and depends on the comments, so that a new one makes it need to run. It replies, and judges whether a comment changes anything. If none does, its output is as before and nothing after it runs again; if one asks something only of the implementation, its output carries that to the implementation; if one changes a product, it refuses that product, with the comment as the reason. The flow says which products it may refuse, and it does not depend on them. The cells before it answer no comments. A comment made while a later cell's run goes on is taken when that run's round ends: the cell runs between two rounds of that run, in the session of its line, and the run then goes on, or is stale where the comment changed what it started from.
Why: The owner comments once a plan can be read, by which time the research and design are mostly done, and every work passes the implementation, so no comment is missed. With one way for a comment to change a work's course, an approval on a question and a "no" in a comment cannot remain side by side unresolved. A reply that waited for a long run to finish would come too late to be of use, and taking the comment between two rounds needs no way of its own: it is the walk's order. Were the cell to depend on the products it may refuse, it would run at every change of the plan with no comment to judge. The cost is accepted: a comment made while a cell before this one rests for a ruling is answered only after the ruling, and one made during a round only when the round ends.
Not: A round only for replying, outside the graph, at every cell where a work may wait.
From: the owner, 2026-11-09

### decision agent-reads-what-it-needs
Besides what every run of its line is given, a run is given its task; a list of what exists on the item and in the work, with each thing's current version and, for each product it may write, the version at which its meaning last changed; and why it runs: the dependencies that changed, or who refused what and why. The content of the item and of the work it reads itself, the principles before the decisions, through the engine's tool for reading, which is a call of the run; the engine records what it read. What a run hands in comes back in its return. The engine keeps no account of what a session already holds.
Why: Only the agent knows what is in its context: a session that goes on remembers the old and reads what changed, a new one reads all. Deciding for it what to send again was a mechanism of its own. The engine's tool is the way in, and files in the worktree are not, because the engine and the machine an agent runs on may be apart, and reading the database goes through one entry the engine controls. Each read costs a round, so the list must let an agent ask for all it needs at once, and what the reads cost is measured. (principles lean-agent-input, measure-speed-and-cost)
From: the owner, 2026-11-09

### decision session-is-a-cache
A session, what a worktree holds uncommitted and the prompt cache are caches: losing one costs money and changes no result. What a later cell needs of an earlier one is in that cell's product. The exception is a cell whose whole job is to load context: its product is the session as it was when the cell finished, kept in the session, and later cells of its line go on in it or fork it. When the engine finds the session lost it records the product as lost, and the cell needs to run; a loss changes no meaning, and rebuilt from the same versions of what it was built on and the same specification, the product is written as a change that does not matter, so nothing after it runs again.
Why: A new session is then the same kind of event as a changed answer, and needs no rule of its own. A cell whose job was other work, run again only to restore what it knew, would find nothing it depends on changed and hand in what it handed in before, reading nothing again; so only a cell whose task is the reading has such a product. (principle keep-what-was-paid-for)
From: the owner, 2026-11-09

### decision every-way-round-has-a-limit
Each thing the flow lets repeat has a limit: how many runs of a cell in a row are ways round, how often a return is sent back within a run, how many calls a run makes. A run is a way round when none of its reasons came from before its cell or from a person: each is a cell's refusal, an answer to a refusal of its own, or a lost product. Once a cell's last runs, as many in a row as its limit, were each a way round and the next would be one, the work waits for the owner, who lets the cell go on, the count starting anew, or cancels the work. A run with a reason from before its cell or from a person starts the count anew. The reasons of every run are recorded as it starts, and the counts are worked out from the records. The compiler refuses a flow in which something that repeats has no limit, or a limit of none.
Why: Something that repeats without a limit spends without end and unseen. In any circle of cells that send each other back, the first of them in the order runs only for what the later ones did, so its count grows whatever the others' reasons are: one count bounds two cells that answer each other with a new version every time, and a cell that keeps rebuilding what is lost. A person's refusal is the owner steering and no circle, so it starts the count anew, and the owner is not asked to let go on what the owner sent back. (principle no-unseen-spending-loop)
Not: Counting a cell's runs since anything before it last changed: that needed a rule for which writes are a change and another for the run the change itself calls for, and it asked the owner to let go on a cell the owner had refused.
From: the owner, 2026-11-09

### decision changed-flow-walks-the-records
A changed flow is installed by walking each work's records on the new graph: a cell that did not change is passed, a new cell has no record and runs when the walk reaches it, a cell taken out leaves runs that count for nothing, and whether a cell whose specification changed keeps its record the owner decides when approving the change. Walking without acting, before the change is approved, shows for each work from which cell it would run again. A record is found by its cell through the table of earlier ids (principle cell-numbering).
Why: Records are kept by cell and by what each run was built on, not by a step in a course taken, so they still fit a graph that has changed. (principle flow-change-spares-works)
From: the owner, 2026-11-09

### decision stops-from-outside-are-no-cells
Cancelling is a fact on the item, pausing a switch on the work that lets no round begin, and how many rounds go on side by side is the scheduler's matter: none of the three is a cell, and a work with no round under way holds no slot.
Why: What stops or holds a work from outside applies wherever the work is, which a cell cannot.
From: the owner, 2026-11-09

## Backend

### decision engine-knows-no-cell
The engine's code names no particular cell: it carries out whatever cells the flow's definition holds. The definition holds each cell's specification (its task, its output, its checks, its line) and how the cells are joined (order, dependencies, conditions, preconditions, what each may write and may refuse, calls, limits, and who rules a dispute). A named test, for a condition or a precondition, is a function in code that declares what it reads; a new kind is a new function. The compiler checks that a dependency names an output of a cell before it, a block of the item or something outside the work; that a cell may refuse only a product some cell may write; that everything that repeats has a limit; that a condition reads only what cells before it, people or the outside wrote; that every named test is one the engine has; and that the merge has no condition.
Why: What is in the definition the owner can read on the chart and the compiler can check before anything runs. (Article 7; principle compile-before-run)
From: the owner, 2026-11-09

### decision one-loop-decides
One loop takes what happens, in order: it adds to the records, and walks each work that has not ended and has no round under way. A round under way is not interrupted: what arrives meanwhile is in the records when the round ends, and the walk taken then decides with it. The rounds of different works go on side by side. The deciding code holds no lock. Records that cannot be those of a work (a run that never started, two entries at one place) are said so by the deciding code, which decides nothing on them.
Why: Decisions taken one after another are the same every time the same things happen in the same order, which is what lets a history be simulated and run again. (principles simulated-histories, nothing-passes-for-normal)
From: the owner, 2026-11-09

### decision a-run-is-one-mechanism
Running a cell is one mechanism, the same for every cell: take the session of the cell's line, starting one where there is none and forking the loaded context where the cell depends on it; give the task; wait for the return; make the calls it asks for and the calls the flow requires; and, when its return has passed, record in one entry what it handed in: its answers, each product it wrote, and for each whether the change matters. A run writes no product before it finishes. A run interrupted by a stop of the engine goes on from what its rounds recorded. Each kind of call says how it starts, when it is complete, and by what it is recognised as already made.
Why: Written once, it is tested once, and a new kind of call touches no cell. A product half written by a run that is then abandoned would have to be told apart, at every reading, from one that counts. (principle effects-known-as-done)
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
No work has two rounds under way at once. A run is running only while one of its rounds is under way: between rounds it is at rest, whatever it rests for (a person's ruling, a tool's result, or only its next round), and a cell before it may run meanwhile. A run at rest that is stale is abandoned, its calls withdrawn and no record left, and its cell then runs again in the same session with what changed among its reasons.
Why: The cells of a work share one worktree and one branch, and a round is the stretch in which an agent or the engine works on them; between rounds nothing does. A run may rest for days for a person, and a comment or a changed answer must be able to reach the work meanwhile: the walk takes what arrived in its order, the cell that takes comments or the writer of a refused product running first. A run that went on from what has since changed would be paid for twice, and the answer to its call would be given for nothing; a comment that changes nothing leaves the run, and the ruling it waits for, as they were. (principle keep-what-was-paid-for)
From: the owner, 2026-11-09

### decision no-merge-against-a-refusal
No work merges while a refusal is in force or a precondition of the merge is not met, and the merge is made only of the commit that the check before it passed.
Why: A merge cannot be taken back, and each of these is someone's "not yet". (principle nothing-passes-for-normal)
From: the owner, 2026-11-09

### decision merge-only-on-what-is-current
At a merge, the record of every cell on the path was built on the current meaning of each thing that cell depends on.
Why: A merge rests on every record before it, and one built on a version that is no longer current carries a decision someone has since changed.
From: the owner, 2026-11-09

### decision a-work-comes-to-rest
When people do nothing more and nothing outside the work changes, every work, after a number of rounds its limits bound, waits, saying for what and for whom, or has ended.
Why: A work that neither waits nor ends is spending. (principle no-unseen-spending-loop)
From: the owner, 2026-11-09

### decision records-give-the-same-walk
Walking a work's records as they were at any moment, on the flow then installed, gives the decision the engine took at that moment.
Why: The page's account of why a work is where it is, the tracing of a fault and the showing of what a changed flow would do all walk the records again. (principle decide-purely)
From: the owner, 2026-11-09

### decision a-stop-changes-nothing
For the same returns of the agents and the same acts of people, a history in which the engine was stopped at any step and started again ends with the same products and the same decisions, each effect made once, as the history without the stop.
Why: An engine that is stopped to be replaced, or that fails, must cost no work and repeat no effect. (principle effects-known-as-done)
From: the owner, 2026-11-09
