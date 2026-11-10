# Designs ahead of the code

Decisions that were designed but are not yet true of the code on main. They are kept out of the design tree, which
holds only what the code does; each becomes an item, or is dropped, and enters the tree with the change that makes it
true. `Gap` says what the code does instead, where there is code.

## Flow

The words these entries use, each with one meaning:

- A work's **records** are all that is recorded of it. A **run** is one execution of a cell for a work, from when it
  starts to when its return has passed everything the flow requires of it; it may hold several **rounds** of the agent
  and several calls. A cell's **record** is its latest finished run: what it handed in, and the versions of what it was
  built on.
- A **product** is a thing with a name and versions that cells read and depend on: a block of the item, an output of a
  cell, or something outside the work, such as main. Its **content version** changes at every write; its **meaning
  version** changes only when the change matters to the cells after it.
- A cell **depends on** a product when a change of its meaning makes the cell need to run.
- A cell is **on the path** of a work when its condition holds on the records as they stand. A cell off the path counts
  as having no record: what it handed in is absent to the cells after it, and refusals by it or against it have no
  force.
- A **refusal** is a cell's or a person's "no" to a version of a product. It **stands** until that product is written
  by a person, from outside the work, or by a run that was given the refusal, which is one that started after the
  refusal was made. A refusal by a cell **has force** while the run that made it is the cell's record and the cell is
  on the path; a person's refusal always has force.
- The **walk** is the engine going through the cells in their order to find what a work does next.
- A **call** is something a run waits for and whose result comes back into it. A **line** is the cells of a work that
  share one session. A **way round** is a path by which a cell's run can lead to the same cell running again: a
  refusal, a return sent back, a call.

### decision work-has-no-position
A work has no stored position. Each time something happens, the engine walks the flow's cells from the first, in their order: it skips a cell that is not on the path; at the first whose preconditions do not hold, whose run waits at a call for a person, or that has reached a limit, the work waits; it runs the first that needs to run, and passes the rest. A precondition holds the work before its cell whether or not the cell needs to run. Before the walk, a run that waited for a person and has its answer goes on, and one that no longer stands is abandoned. A work has ended when its last cell has a record or its item is cancelled, and an ended work is not walked; the last cell of a flow has no condition. What is stored is the records, among them who refused what; where the work goes next is walked.
Why: A stored position has to be read rightly by every rule, and a way back written by hand has to say where to, with what, and what of the earlier work still counts. Two defects of an engine that stored a position were both there: a work sent back without end once its cell asked for a tool, and a way back judged by the number of each cell's first round. Walking from the first cell also takes first whatever else changed meanwhile: when the owner changes an answer after the plan was implemented, the plan is redone before anything built on it runs again. (principles state-from-records, decide-purely)
Not: Hierarchical state machines, which end the repeated rows but keep a position: a person's input stays an interruption to be hung on every state, and a changed flow still needs a place to land each work.
From: the owner, 2026-11-09

### decision cell-runs-when
A cell needs to run when it has no record, or when one of these is true of its record: a product it wrote is gone; something it depends on has changed in meaning since; a refusal it made has been answered; or a refusal that has force stands against a product it wrote. A version written by the cell's own record never makes it need to run for a change of meaning.
Why: These are all there is to going on and going back: a first run, a rerun because what it was built on changed or was lost, a rerun to judge again, a rerun because its product was judged wanting. Running again every cell after one that ran again was refused as the rule: a design redone need not mean an implementation redone when what the implementation depends on did not change. (Article 3)
From: the owner, 2026-11-09

### decision dependencies-declared
Each cell declares what its result depends on: an output of another cell, blocks of the item, or something outside the work such as main or the project's settings. What a cell reads without declaring it never makes it need to run. A dependency names a cell's output, not the branch as it stands, where that output is what the cell judged. Where a cell must keep to something outside as it was at a moment, a cell writes that moment down as a product and the other depends on the product: the cell that starts a work writes the commit of main its branch starts from, each time it runs; the cell that reads the tree depends on that, and the merge depends on main itself.
Why: Whether a cell must run again is then worked out from what it was built on, by one rule and one kind of dependency. A review that depends on what the implementation handed in is not started over by every commit after it.
Not: A second kind of dependency that keeps the version a cell first ran on: an item given a new dependency must have its research run again on the main it then starts from, which a kept version would never allow.
From: the owner, 2026-11-09

### decision meaning-told-by-the-writer
An output that is a choice or a list of names changes its meaning when its value does, whoever writes it. Prose changes its meaning when its writer says so: with each write the cell says, with a line of why, whether the change matters to the cells after it, compared with the version at which the meaning last changed, which it is given. Prose written by a person, or from outside the work, always changes the meaning. A product written anew in answer to a refusal makes its refuser need to run, whatever its writer says; other cells go by the writer's word. The answer and its reason are kept where the owner sees them, and the page marks where what the owner was last shown differs from what stands now.
Why: An agent never writes the same text twice, so judging prose by its text would run every later cell again after every rerun: when an item's title is reworded and the research adjusts a few words of its plan, nothing after it should run. Comparing with the version at which the meaning last changed keeps a row of small changes, each said not to matter, from adding up unseen. An answer to a refusal must reach the one who refused. The owner reads the designs where this arises, and the page's mark shows exactly the cases where a change was said not to matter. (Article 3)
Not: A second agent to check the answer, at a round for every change.
From: the owner, 2026-11-09

### decision refusal-runs-the-writers-again
While a refusal stands and has force, every cell on the path that wrote a version of the product needs to run, the earliest first, and every run that writes the product is given who refused and why. A writer that reruns and leaves the product as it was is run again, up to its limit. A cell that leaves to a later cell a product it wrote must withdraw it, which the engine checks on its return; a withdrawal is a write, and not the product being gone. Grounds for disagreeing are a write too, and make the refuser judge again; grounds the refuser does not accept go, as a call of the writer's next run, to whoever the flow names to decide between them, and both depend on the ruling.
Why: A refusal says only what is wrong, and the walk finds where the work goes: no way back says where to. A run that was not given a refusal cannot have answered it, so only a write after it was given ends it. That a cell which does not change what was refused is run again until its limit, and the work then put before the owner, is how an answer of the owner's is kept from being passed over. (principle reviews-can-be-disputed)
From: the owner, 2026-11-09

### decision conditions-and-preconditions
A cell, or a stretch of consecutive cells, may have a condition; a cell may have preconditions. A condition says when it is on the path: where an agent's choice decides it, by the yes/no answer of the cell right before (principle yes-no-forks); otherwise by a named test of what cells before it, people or the outside wrote. The compiler refuses a condition that reads the cell's own output or a later cell's. A precondition says when the work must wait before the cell; it is a named test of the records as they stand, worked out at every walk and never stored beside them. A gate is no cell but a precondition of the cell it guards, and what a work waits for, and who can give it, is the precondition not met.
Why: A fork drawn as rows and a gate drawn as a cell each needed a record of what the work was waiting for, and care to raise and withdraw that record at the right moments. A test of the records as they stand cannot be left standing after its cause is gone. A condition that read what its own cell or a later one handed in would decide whether a cell is on the path by what only being on the path produces. (principle state-from-records)
From: the owner, 2026-11-09

### decision calls-inside-a-run
What a cell asks for while it runs (a tool, a pause, a ruling from whoever can decide when it cannot go on), and what the flow requires its return to pass (the engine's checks, a review that only advises), are calls of that run: the result comes back into the run, and the run ends when its return has passed. A call holds no place in the order of cells; the flow declares it on the cell, and the chart draws it beside the cell. A call the flow requires is made by the engine, never left to the agent to ask for.
Why: Drawn as cells, three kinds of call took some thirty-seven of an engineer graph's ninety-four rows, the same three on every agent cell, and the engine kept a stack of who had called whom; where a work stood then had two answers, the tool's cell and the cell the tool ran for. A review that advises once, and whose remarks return to the cell that asked, is the same thing as a tool. What a task only asks of an agent, the agent may leave undone, so a required call is the flow's.
From: the owner, 2026-11-09

### decision a-person-judges-outside-the-order
The owner's confirmation of the answers to an item's questions is a judgment made at any moment, outside the order of cells. An answer other than the one recommended, or an objection, refuses the question as it stands. A confirmation is of the question's current meaning, and holds while that meaning stands.
Why: A person cannot be scheduled, so the judgment is a record that arrives when it arrives, under the rule for any refusal. Refusing the question, not one writer, leaves no choice of where to send the work.
From: the owner, 2026-11-09

### decision comments-through-one-cell
A comment never steers a work by itself. One cell, placed before the implementation, takes the comments, and a comment is not put into a round that is running: the cell is on the path once the item has a comment, which is a test of what a person wrote, and depends on the comments, so that a new one makes it need to run. It replies, and judges whether a comment changes anything. If none does, its output is as before and nothing after it runs again; if one asks something only of the implementation, its output carries that to the implementation; if one changes a product, it refuses that product, with the comment as the reason. The cells before it answer no comments.
Why: The owner comments once a plan can be read, by which time the research and design are mostly done, and every work passes the implementation, so no comment is missed. With one way for a comment to change a work's course, an approval on a question and a "no" in a comment cannot stand side by side unresolved. The cost is accepted: a comment made while a cell before this one waits for a ruling is answered only after the ruling.
Not: A round only for replying, outside the graph, at every cell where a work may wait.
From: the owner, 2026-11-09

### decision agent-reads-what-it-needs
Besides what every run of its line is given, a run is given its task; a list of what exists on the item and in the work, with each thing's version and, for each product it may write, the version at which its meaning last changed; and why it runs: the dependencies that changed, or who refused what and why. The content of the item and of the work it reads itself, the principles before the decisions, through the engine's tool for reading, which is a call of the run; the engine records what it read. What a run hands in still comes back in its return. The engine keeps no account of what a session already holds. When this is built, the first rule of `design/README.md` changes with it, to say that every agent reads the constitution and the principles itself.
Why: Only the agent knows what is in its context: a session that goes on remembers the old and reads what changed, a new one reads all. Deciding for it what to send again was a mechanism of its own. The engine's tool is the way in, and files in the worktree are not, because the engine and the machine an agent runs on may be apart, and reading the database goes through one entry the engine controls. Each read costs a round, so the list must let an agent ask for all it needs at once, and what the reads cost is measured. (principles lean-agent-input, measure-speed-and-cost)
From: the owner, 2026-11-09

### decision session-is-a-cache
A session, what a worktree holds uncommitted and the prompt cache are caches: losing one costs money and changes no result. What a later cell needs of an earlier one is in that cell's product. The exception is a cell whose whole job is to load context: its product is the session as it stood when the cell ended, kept in the session, and later cells of its line go on in it or fork it. When the engine finds the session lost it records the product as gone, and the cell needs to run; rebuilt from the same versions of what it was built on and the same specification, the product keeps its meaning, so nothing after it runs again.
Why: A new session is then the same kind of event as a changed answer, and needs no rule of its own. A cell whose job was other work, run again only to restore what it knew, would find nothing it depends on changed and hand in what it handed in before, reading nothing again; so only a cell whose task is the reading has such a product. (principle keep-what-was-paid-for)
From: the owner, 2026-11-09

### decision every-way-round-has-a-limit
Each way round has a limit: how often a cell is run again for refusals, how often a return is sent back within a run, how many calls a run makes. A cell whose only reasons to run are refusals waits for the owner once it has been run again, while a refusal by the same cell or person stood against what it wrote, as often as its limit, counted since the owner last let it go on. At a limit the work waits for the owner, who lets it go on or cancels it. The counts are worked out from the records. The compiler refuses a flow in which a way round has no limit.
Why: A way round without a limit spends without end and unseen. Counting by who refuses, and not by each refusal, bounds two cells that answer each other with a new version every time. (principle no-unseen-spending-loop)
From: the owner, 2026-11-09

### decision changed-flow-walks-the-records
A changed flow is installed by walking each work's records on the new graph: a cell that did not change is passed, a new cell has no record and runs when the walk reaches it, a cell taken out leaves runs that count for nothing, and whether a cell whose specification changed keeps its record the owner decides when approving the change. Walking without acting, before the change is approved, shows for each work from which cell it would run again. A record is found by its cell through the table of earlier ids (principle cell-numbering).
Why: Records are kept by cell and by what each run was built on, not by a step in a course taken, so they still fit a graph that has changed. (principle flow-change-spares-works)
From: the owner, 2026-11-09

### decision stops-from-outside-are-no-cells
Cancelling is a fact on the item, pausing a switch on the work, and how many runs go on side by side is the scheduler's matter: none of the three is a cell, and a work that waits holds no slot.
Why: What stops or holds a work from outside applies wherever the work stands, which a cell cannot.
From: the owner, 2026-11-09

## Backend

### decision engine-knows-no-cell
The engine's code names no particular cell: it carries out whatever cells the flow's definition holds. The definition holds each cell's specification (its task, its output, its checks, its line) and how the cells are joined (order, dependencies, conditions, preconditions, what each may refuse, calls, limits, and who rules a dispute). A named test, for a condition or a precondition, is a function in code that declares what it reads; a new kind is a new function. The compiler checks that a dependency names an earlier cell's output, a block of the item or something outside the work; that a cell refuses only a product it depends on; that every way round has a limit; that a condition reads only what cells before it, people or the outside wrote; and that the last cell has no condition.
Why: What is in the definition the owner can read on the chart and the compiler can check before anything runs. (Article 7; principle compile-before-run)
From: the owner, 2026-11-09

### decision one-loop-decides
One loop takes what happens, in order: it adds to the records, and walks each work that has not ended, is not paused and has no cell running: a round under way is not interrupted when what it depends on changes, and what it hands in is then judged by the next walk. The runs of different works go on side by side. The deciding code holds no lock.
Why: Decisions taken one after another are the same every time the same things happen in the same order, which is what lets a history be simulated and run again. (principle simulated-histories)
From: the owner, 2026-11-09

### decision a-run-is-one-mechanism
Running a cell is one mechanism, the same for every cell: take the session of the cell's line, starting one where there is none and forking the loaded context where the cell depends on it; give the task; wait for the return; make the calls it asks for and the calls the flow requires; and record what it handed in, the versions it was given or read, and what it said of each change's meaning. A run interrupted by a stop of the engine goes on from what its rounds recorded. Each kind of call says how it starts, when it is complete, and by what it is recognised as already made.
Why: Written once, it is tested once, and a new kind of call touches no cell. (principle effects-known-as-done)
From: the owner, 2026-11-09

### decision engine-in-layers
The engine's code is in layers, each depending only on those before it: the deciding code, which depends on the definitions alone; storage; the real implementations of the interfaces; the loop; the owner's interface.
Why: The order is the order of what can be tested alone: the deciding code with no storage, storage with no agent, the loop with test implementations of everything outside it. (principle dependencies-one-way)
From: the owner, 2026-11-09

### decision tables-only-added-to
The tables the deciding code is given are only added to, each row with its place in the order of what happened. There is no general store of events beside them.
Why: The deciding code is given a work's records as they stood at a moment, and tables only added to give that with no copy kept. A general store of events would be a mechanism with no second use. (principles state-from-records, generalise-at-the-second-use)
From: the owner, 2026-11-09

## What the engine always keeps

The properties the simulated histories check after every step (principle simulated-histories).

### decision one-cell-of-a-work-at-a-time
No work has two cells running at once. A run that waits at a call for a person is not running: a cell before it may run meanwhile. The waiting run is abandoned, its call withdrawn and no record left, once something it depends on has changed in meaning since the run started, or its cell is no longer on the path.
Why: The cells of a work share one worktree and one branch. A run that waits for a person may wait for days, and a comment or a changed answer must be able to reach the work meanwhile; a comment that changes nothing must not take the owner's pending ruling away.
From: the owner, 2026-11-09

### decision no-merge-against-a-refusal
No work merges while a refusal that has force stands against a product a cell on its path wrote, or a precondition of the merge is not met.
Why: A merge cannot be taken back, and each of the two is someone's "not yet". (principle nothing-passes-for-normal)
From: the owner, 2026-11-09

### decision merge-only-on-what-stands
At a merge, every product the merge depends on, directly or through other products, was built on the current meaning of each thing it depends on.
Why: A merge rests on every product before it, and one built on a version that no longer stands carries a decision someone has since changed.
From: the owner, 2026-11-09

### decision a-work-comes-to-rest
When people do nothing more and nothing outside the work changes, every work, after a number of runs its limits bound, waits, saying for what and for whom, or has ended.
Why: A work that neither waits nor ends is spending. (principle no-unseen-spending-loop)
From: the owner, 2026-11-09

### decision records-give-the-same-walk
Walking a work's records as they stood at any moment, on the flow and the configuration then installed, gives the decision the engine took at that moment.
Why: The page's account of why a work stands where it does, the tracing of a fault and the showing of what a changed flow would do all walk the records again. (principle decide-purely)
From: the owner, 2026-11-09

### decision a-stop-changes-nothing
For the same returns of the agents and the same acts of people, a history in which the engine was stopped at any step and started again ends with the same products and the same decisions, each effect made once, as the history without the stop.
Why: An engine that is stopped to be replaced, or that fails, must cost no work and repeat no effect. (principle effects-known-as-done)
From: the owner, 2026-11-09
