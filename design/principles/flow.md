### principle one-flow
There is one flow, compiled into the program, for every project; what a project needs differently goes in its configuration.
Why: A flow per project brings switches on the page, batch updates of nearly identical flows, and forked versions. (Article 1)

### principle state-is-blocks
A cell is joined to the other cells only through the blocks of the work's item: it reads its inputs, and what it hands in goes into the item as its outputs. A cell runs again only when one of its inputs has changed in meaning since it last ran, by the same rule whoever wrote the change (a cell before it or after it, a person, the outside).
Why: The same inputs give the same outputs, and one rule then covers going on and going back: a first run, a rerun, a fix because someone objected, a second look because the fix changed what was judged. Cells wired output to input had to be rewired for every cell added, a way back written by hand had to say where to, with what, and what of the earlier work still counted, and a "no" kept as a thing of its own needed rules for who answers it, when it ends and whether it still has force. (principle state-from-records; Article 4)

### principle compile-before-run
A changed flow is compiled before it is installed, and one that fails the compiler's checks never reaches a running work.
Why: An error found halfway through a work costs far more than one found at compile time.

### principle cell-numbering
A cell's id is its stage's initial and a number, its name staying as its title. A stage numbers its cells in running order with no gap; a cell added or removed renumbers those after it, and the table of earlier ids keeps each recorded round reading as the cell it ran at.
Why: A short id is easier to type than a name, and its number tells where the cell runs; a number out of order or a gap tells nothing and is read as a missing cell.

### principle cache-order
What is sent to an agent is ordered from the widest scope to the narrowest: what every cell shares first, what changes every round last. Which part each thing belongs to is declared in the flow definition, not computed in code, and every change of the order is confirmed from the cache reads and writes of real rounds.
Why: The prompt cache matches only the leading bytes of a request, so only an identical beginning is read from it. One order that should have hit missed because of a detail no one noticed, and content placed after the varying part was rewritten for every work, a fifth of the cost of early research. An order computed in code is invisible to anyone reading the flow. (Article 7)

### principle yes-no-forks
Where an agent decides which way the work goes, the fork is one yes/no question asked by the cell right before it, answered by the facts, never by caution, and the way on depends on that answer alone. Being unable to go on is not a fork: the work comes back to the same cell.
Why: A label kept for later routing compresses information, so judging by it later distorts, and it hides steps no one sees. "When unsure, take the heavier path" makes works that do not need it pay for it.

### principle locate-and-price-the-fix
Before something is added to the flow, the problem is placed, every work or only some kind of work, and a cell that serves every work never asks any work for an answer only some works have. A mistake is stopped first by rewriting a sentence of the cell's input; a question (an answer from every work), or a check, a second pair of eyes or a cell (a round of every work), is added only where the input cannot stop it or agents get it wrong even when told, and what it stops is worth that cost.
Why: Things added where a problem showed up each looked reasonable alone, and together caused most of the flow's problems. Models follow only part of what they are told, so some mistakes need a guard; but a guard costs every work, and as models improve it is worth less. Making small things big or big things small, or stopping agents from doing what they could, is worse than a bug in the flow. (From the flow's audits; not measured.)

### principle focused-judgment-alone
A judgment that needs research and focus, such as whether something is a design defect, is asked as a step of its own, not as a side question in another task, and not every time; agents elsewhere may still make it.
Why: Asked on the side, it is not thought through (experience, not measured). Making other agents report only signs and never judge limits them for no reason.

### principle reviews-can-be-disputed
The side that did the work may dispute a review's finding, and a dispute the review does not accept goes to whoever decides between them.
Why: Reviews are not always right, and a wrong finding that cannot be disputed forces a wrong change. (Article 9)

### principle change-cells-after-trials
A cell's prompt, rules, inputs or outputs change only for a problem that can be named, and a change takes effect only after the changed cell has run on a few real past cases, covering every way a work can enter it, with its inputs and returns compared to the old cell's case by case. What has no serious problem is left alone.
Why: Changes made by reasoning alone made cells worse than before, and changing a cell that works puts at risk what it already does well. A cell tried from one direction breaks when entered from another.

### principle trial-round-by-round
A changed flow is tried one round at a time: before the next round runs, its inputs are read (their order, their size, and nothing given that the session already held), its return and route, and its token and cache numbers; then a fork asks about the cell's text.
Why: A hundred and one rounds run in a row once missed a problem that was there in the first; a round's numbers show an input given twice or a cache missed, which its return does not.

### principle ask-in-a-fork
What an agent thinks of a cell is asked after its round, in a fork of its session with no tools and nothing of the work changed; the round itself runs as usual, and the agent is not told it is being tested. The fork is asked only about the cell's text: which words can be read another way, what is missing or unneeded, how to fix each, quoting the text.
Why: An agent that knows it is being tested does not do what it would in real work (an inference, not measured). The return cannot show which words can be read another way; the fork, holding everything the round saw, can. Asking why it acted as it did is refused whole by the model's safety check (it happened on 2026-11-08).

### principle audit-cell-by-cell
The flow is audited or redone one cell at a time, each finished before the next, starting from the problem the cell is there to stop and the ideal cell, with existing cells and recorded rounds only as evidence; the cell's decision says what problem it stops, how, why by that means and at what cost. A cell is frozen once the owner has reviewed it as it stands in the definition, on the chart, and only the owner reopens it.
Why: Cells added one at a time, each reasonable alone, drift apart, and only going through all of them shows it; cells settled in a draft came out different in the definition. With each cell's problem and cost written down, taking a capability from a cell shows which problem must be met elsewhere.

### principle flow-tests-formal
The flow's routing is kept right by tests in the project's suite, which run at every change, not by trying the flow once; trials on past cases judge a cell's text, not its routing.
Why: A trial goes one way through the flow; the next change then meets a way no one ever ran.

### principle flow-change-spares-works
A flow change is made so that returns already handed in still pass where it can, and what installing it does to each work in progress is shown before it is approved.
Why: Every tightening of an early cell's output sends each work that went past it back to do the cell again; the owner approves a flow change with that cost in view. (Article 3)
