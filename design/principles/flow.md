### principle one-flow
There is one flow, compiled into the program, for every project; what a project needs differently goes in its configuration.
Why: A flow per project brings switches on the page, batch updates of nearly identical flows, and forked versions. (Article 1)

### principle compile-before-run
A changed flow is compiled before it is installed, and one that fails the compiler's checks never reaches a running work.
Why: An error found halfway through a work costs far more than one found at compile time.

### principle cell-numbering
A cell's id is its stage's initial and a number, its name staying as its title. A stage numbers its cells in running order with no gap; a cell added or removed renumbers those after it, and the table of earlier ids keeps each recorded round reading as the cell it ran at.
Why: A short id is easier to type than a name, and its number tells where the cell runs; a number out of order or a gap tells nothing and is read as a missing cell.

### principle cache-order
What is sent to an agent is ordered from the widest scope to the narrowest: what every cell shares first, what changes every round last. Which part each thing belongs to is declared in the flow definition, not computed in code, and every change of the order is confirmed from the cache reads and writes of real rounds.
Why: The prompt cache matches only the leading bytes of a request, so only an identical beginning is read from it. One order that should have hit missed because of a detail no one noticed, and content placed after the varying part was rewritten for every work, a fifth of the cost of early research. An order computed in code is invisible to anyone reading the flow. (Article 7)

### principle the-system-prompt-holds-what-a-line-shares
The system prompt of a line's session holds only what every cell of the line is given alike and what is the same in every session of the line. Everything else is sent in a message as its cell is entered.
Why: The agent's program takes a session's system prompt once, as the session begins, and keeps it; one given again when the session is taken up is ignored. So what is put there reaches every cell of the line and can be neither changed nor added to until the session is reopened. And two sessions share it in the cache only where it is the same to the byte: a difference anywhere in it loses all of it.

### principle one-form-of-return-for-a-session
The form of a return given to the agent's program is the same on every call of a session, which is a line's: it holds the engine's fields, the tools the flow names, and the blocks the cells of the line write, each in its own shape. It is never made for one cell: where a cell needs a shape the form does not give, the cell is told that shape in its message, and the engine checks it as it reads the return. Each cell is told which blocks are its own with its task, and the engine sends back a return that writes a block that is not its cell's.
Why: The agent's program puts the form with the tools at the very front of every request, before the system prompt and the session's messages, and does not keep it with the session as it keeps the system prompt; so a form that differs from the call before leaves nothing of the session in the cache (principle cache-order). Measured in one session, a call with the form of the call before read 77,555 tokens from the cache and wrote 248, and the call whose form differed read none and wrote 38,961.

### principle yes-no-forks
Where an agent decides which way the work goes, the fork is one yes/no question asked by the cell right before it, answered by the facts, never by caution, and the way on depends on that answer alone. Being unable to go on is not a fork: the work comes back to the same cell.
Why: A label kept for later routing compresses information, so judging by it later distorts, and it hides steps no one sees. "When unsure, take the heavier path" makes works that do not need it pay for it.

### principle locate-and-price-the-fix
Before something is added to the flow, the problem is placed, every work or only some kind of work, and a cell that serves every work never asks any work for an answer only some works have. A mistake is stopped first by rewriting a sentence of the cell's input; a question (an answer from every work), or a check, a second pair of eyes or a cell (a round of every work), is added only where the input cannot stop it or agents get it wrong even when told, and what it stops is worth that cost.
Why: Things added where a problem showed up each looked reasonable alone, and together caused most of the flow's problems. Models follow only part of what they are told, so some mistakes need a guard; but a guard costs every work, and as models improve it is worth less. Making small things big or big things small, or stopping agents from doing what they could, is worse than a bug in the flow. (From the flow's audits; not measured.)

### principle a-flow-serves-every-kind-of-work
A flow is designed for every kind of work its works bring, never around one of them: the engineer's flow serves a defect, a capability not there yet, a change of design, a measurement and a cleanup alike, and no cell's words take one kind for the work, nor give a step only one kind can take as every work's first.
Why: Words written for one kind send every other kind down a path that does not fit it. The engineer's research was told to reproduce a defect first; an item that adds a capability had no defect, the words had nothing for its first step, and read literally they would stop it to ask the owner (the trials of 2026-11-14). The like cases of a defect and how to keep its kind from coming back were struck from the engineer's words before, for the same reason.

### principle focused-judgment-alone
A judgment that needs research and focus, such as whether something is a design defect, is asked as a step of its own, not as a side question in another task, and not every time; agents elsewhere may still make it.
Why: Asked on the side, it is not thought through (experience, not measured). Making other agents report only signs and never judge limits them for no reason.

### principle reviews-can-be-disputed
The side that did the work may dispute a review's finding, and a dispute the review does not accept goes to whoever decides between them.
Why: Reviews are not always right, and a wrong finding that cannot be disputed forces a wrong change. (Article 9)

### principle change-cells-after-trials
A cell's prompt, rules, inputs or outputs change only for a problem that can be named, and a change takes effect only after the changed cell has run on a few real past cases, covering every way a work can enter it, with its inputs and returns compared to the old cell's case by case. What has no serious problem is left alone.
Why: Changes made by reasoning alone made cells worse than before, and changing a cell that works puts at risk what it already does well. A cell tried from one direction breaks when entered from another.

### principle audit-cell-by-cell
The flow is audited or redone one cell at a time, each finished before the next, starting from the problem the cell is there to stop and the ideal cell, with existing cells and recorded rounds only as evidence; the cell's decision says what problem it stops, how, why by that means and at what cost. A cell is frozen once the owner has reviewed it as it stands in the definition, on the chart, and only the owner reopens it.
Why: Cells added one at a time, each reasonable alone, drift apart, and only going through all of them shows it; cells settled in a draft came out different in the definition. With each cell's problem and cost written down, taking a capability from a cell shows which problem must be met elsewhere.

### principle flow-change-spares-works
A flow change is made so that returns already handed in still pass where it can, and what installing it does to each work in progress is shown before it is approved.
Why: Every tightening of an early cell's output sends each work that went past it back to do the cell again; the owner approves a flow change with that cost in view. (Article 3)

### principle a-defect-is-seen-before-it-is-fixed
A defect is fixed only after it was seen to happen where it shows, such as on the page in a real browser or in the
program run on its own data; where no test can be written for it, a probe, a log or the owner running it may show it.
One that is seen nowhere, after all the tools reach was tried, stops the work at its research and goes to no deeper
research: how to go on is written as a question the work waits on, to fix by the cause inferred, to wait for the owner
to give the conditions, or to close the item.
Why: A cause that was only inferred was once fixed and passed its test in a stand-in for the browser, while the defect
stayed and came back as a new item; and it is not known then whether the tools fall short, which a deeper research
shares.

### principle the-reviewer-knows-no-less
Whoever reviews is no less able than the engineer and knows what the engineer knows: the background and the decisions
the engineer worked from.
Why: Engineers mostly do what a review says, so a wrong demand turns good code into bad.

### principle every-plan-is-reviewed-before-it-is-built
Every work's plan is reviewed before it is built, for its design and against the design tree, by a session that did
not write it, whatever the size of the change; a change too small to be worth that is joined with related ones into
one work, never run alone.
Why: A small change left unreviewed can break what matters, and the damage is not small (a judgment, with no case
behind it). A fix of a few lines that ran the flow alone took an hour and more with the waiting; what answers that is
joining small things into one work, not leaving out the review.

### principle a-question-is-a-choice
What is asked of a person is a question of yes or no, or a choice of one or more among options, with the answer its
asker recommends. The answer is the option taken and nothing else: why, where the owner wants to say it, is said in
the owner's words to the cell. What the engine itself asks, such as an approval the flow requires, judges nothing and
recommends nothing.
Why: A question in free words cannot be answered quickly, and words given with an answer, taken as part of it, could
not be told from what the owner says to the cell.

### principle a-question-says-first-what-to-decide
A question an agent puts to the owner says in its first sentence what is to be decided and what it affects, uses no
inner names, gives what follows from each option and a recommendation, and keeps its details behind a link; it is
short, and is opened only where it is not understood.
Why: Otherwise the owner reads the whole story again each time; a question written well can be decided with nobody's
help.

### principle who-asks-can-be-spoken-to
A cell that may ask the owner a question can be spoken to by the owner; a cell that asks nothing is not spoken to.
Why: Once asked, the owner may need to ask back about the question's background, and only the cell that asked knows why
it asked: said to another cell, the words reach one that does not know the question. A cell that asks nothing has
nothing of its own the owner needs to take up with it: what the owner wants of a review goes through the cell that
answers the review, which fixes the point or has the owner rule on it.

### principle named-paths-wait-for-the-owner
A change that touches a path the project's configuration names for the owner, such as a specification or the
principles of its design, waits at the merge for the owner's yes, and a later change to those paths asks again. The
engine sees the paths touched and asks by itself, showing the change; which paths, each project's configuration says,
and the flow names none.
Why: A rule in the project's own guide, left to the engineer and the review to keep, was turned down: where both
missed it, the change merged (Article 7). The paths are a project's and not the flow's (Article 1).

### principle the-flow-gives-the-trees-rules
The rules of the design tree, the tree's own file of rules, are data the cell of every agent takes, as the project's
guide is (the design of data and context): the flow gives them, and no cell is left to find them by following the
guide. The guide points to them as well, for the agents that work outside the flow.
Why: Every project's agents need the same rules, and said once in the flow they reach every cell, where a rule an
agent must find by itself may go unread. An agent outside the flow is given nothing by the flow and has only the
guide.

### principle an-agent-knows-only-its-task
An agent works from its task and the data it is given, and knows nothing of the flow around it: not the cell it runs
as, not the other cells, not who reads what it writes. A cell's words name no other cell.
Why: Carrying what one cell writes to the next is the flow's work (principle mechanical-engine). Words that tell an
agent about the flow copy the flow, go stale when it changes, and invite the agent to do the flow's work.

### principle a-cell-knows-only-its-data
A cell knows nothing of the other cells: it is given only the data it watches and reads, and is run only by a version
of a datum it watches that is new to it, which a version it wrote never is (design document `data-and-context`, which
says what else is new to no cell). Nothing else runs a cell, and no cell is excepted: no trigger apart from the data,
no datum made only to run one, no version sent to some of the cells that watch it and kept from others, and what a
cell hands in rests on what it is given and on the checkout at the versions it is given; a need this cannot meet is a
fault of the design, mended there (principles solve-within-the-design-first and engine-knows-no-cell).
Why: Triggers kept apart from the data grew into a second graph to keep consistent with the first: data with no
content made for each cell to be woken, a datum and a trigger for each pair of cells that answer each other, and
versions sent on by who wrote them. Each met one case; together they made a work's way through the flow something the
owner, watching one work go back from the implementation through the plan's cells, had to ask about at each of three
steps. This is the flow's side of principle an-agent-knows-only-its-task.

### principle one-dialogue-for-each-party
Each agent's cell that judges or doubts another cell's work has a dialogue of its own with that cell, and so has the
owner with each cell it speaks to: one datum only the two parties write, so that each word is new to the other party
alone (design document `data-and-context`). An act of the engine reads no words and is no party: it is answered by
what runs it again (decision an-act-is-answered-by-what-runs-it).
Why: One record for several parties woke every party at each answer, and a datum made for each answer doubled the
data and the triggers of every pair, growing with each cell added.

### principle the-one-woken-owes-the-next-word
In an open dialogue the party woken owes the next word, and a judge's pass, which closes the dialogue, is written as
its verdict alone (design document `data-and-context`). The engine sends back a return that plainly leaves the word
owed, as the document says when it does.
Why: Were every side to answer whenever it is woken, a judge could never pass without a word, and a pass with a word
would wake the maker for nothing; owing the next word only in an open dialogue holds both (a judgment, with no case
behind it). A side left owing with nothing to send it back stalls the work with nobody woken, as a red check answered
in words alone did (decision an-act-is-answered-by-what-runs-it).
