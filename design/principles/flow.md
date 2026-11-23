# The flow

How the flow is designed, changed and tried, and what its agents are given and asked.

### principle cell-numbering
A cell's id is its stage's initial and a number, its name staying as its title. A stage numbers its cells in running order with no gap; a cell added or removed renumbers those after it, and the table of earlier ids keeps each recorded round reading as the cell it ran at.
Why: A short id is easier to type than a name, and its number tells where the cell runs. Numbers that were never reused grew large, left gaps and ran out of order, and a gap is read as a missing cell.

### principle cache-order
What an agent is sent runs from the widest scope to the narrowest: what every cell shares first, what changes each round last. The flow's definition, not the code, says which part each thing is in, and each change of the order is checked against the cache reads and writes of real rounds.
Why: The prompt cache reads only an identical start of a request. One order that should have hit missed by a detail no one saw, and content put after the varying part was written again for every work, a fifth of early research's cost. An order set in code is hidden from readers of the flow. (Article 7)

### principle the-system-prompt-holds-what-a-line-shares
The system prompt of a line's session holds only what every cell of the line is given alike and what is the same in every session of the line. Everything else is sent in a message as its cell is entered.
Why: The agent's program reads a session's system prompt once, as it starts, and ignores one given again on resuming. So what is there reaches every cell of the line, and it cannot be changed or added to until the session reopens. Two sessions share it in the cache only if it is identical to the byte.

### principle one-form-of-return-for-a-session
Each call in a line's session gives the agent's program the same return form: the engine's fields, the flow's tools and the blocks the line's cells write. No cell gets its own form: its message names its blocks and any other shape it needs, and the engine sends back a return that breaks either.
Why: The agent's program puts the form, with the tools, first in every request and does not keep it with the session, so a form unlike the last call's loses the session's whole cache. In one session, the same form read 77,555 cached tokens and wrote 248; a changed one read none and wrote 38,961.

### principle locate-and-price-the-fix
A question, check, reviewer or cell costs every work, so it is added to the flow only where rewording the input does not stop the mistake and what it stops is worth that cost.
Why: Things added case by case each looked fine and together caused most flow problems. Agents miss part of what they are told, so some mistakes need a guard; each costs every work and is worth less as models improve. Sizing things wrongly or holding agents back is worse than a flow bug. (Not measured.)

### principle a-flow-serves-every-kind-of-work
A flow is designed for every kind of work its works bring, never around one: the engineer's flow serves a defect, a missing capability, a design change, a measurement and a cleanup alike, and no cell's words take one kind for the work or give every work a first step only one kind can take.
Why: Words for one kind send every other down the wrong path. Research told to reproduce a defect first had no first step for an item adding a capability, and read literally would stop to ask the owner. For the same reason, the engineer's words no longer ask for similar defects or how to stop their kind.

### principle focused-judgment-alone
A judgment that needs research of its own, such as whether a fault is a design defect, is not asked as a side question in a task about something else; an agent elsewhere is still free to make it.
Why: Asked on the side, it is not thought through (experience, not measured). Making other agents report only signs and never judge limits them for no reason.

### principle reviews-can-be-disputed
The side that did the work may dispute a review's finding, and a dispute the review does not accept goes to whoever decides between them.
Why: Reviews are not always right, and a wrong finding that cannot be disputed forces a wrong change. (Article 9)

### principle change-cells-on-what-their-forks-bear-out
A cell's prompt, rules, data or outputs change only for a problem a real round showed, at the words a fork of it bears out; what has no serious problem is left alone. A change takes effect once a fork from that moment reads the new words as meant and, given the round again, does what the cell means.
Why: Changes made by reasoning alone made cells worse than before, and changing a cell that works puts at risk what it already does well. In work 11 a fork bore out two of three guesses at a fault and showed the third wrong.

### principle audit-cell-by-cell
The flow is audited or redone one cell at a time, each finished before the next, starting from the problem the cell stops and the ideal cell, with existing cells and recorded rounds only as evidence. A cell the owner has reviewed as it stands on the chart is frozen until he reopens it.
Why: Cells added one at a time, each reasonable alone, drift apart, and only going through all of them shows it; cells settled in a draft came out different in the definition. With each cell's problem and cost written down, taking a capability from a cell shows which problem must be met elsewhere.

### principle flow-change-spares-works
A flow change applies to the cells a work has not yet passed; a cell a work has already passed is not run again because its definition changed.
Why: What a work handed in was right by the flow it ran on; running it again for a change of the flow spends rounds on work that is done. (Article 3)

### principle a-defect-is-seen-before-it-is-fixed
A defect is fixed only once seen where it shows, as in a real browser or the program on its own data, by a test or else a probe, a log or the owner. If no tool shows it, the work stops at research, going no deeper, and asks the owner: fix the inferred cause, give the conditions, or close the item.
Why: A cause that was only inferred was once fixed and passed its test in a stand-in for the browser, while the defect stayed and came back as a new item; and it is not known then whether the tools fall short, which a deeper research shares.

### principle the-reviewer-knows-no-less
Whoever reviews is no less able than the engineer and knows what the engineer knows: the background and the decisions the engineer worked from.
Why: Engineers mostly do what a review says, so a wrong demand turns good code into bad.

### principle every-plan-is-reviewed-before-it-is-built
Every work's plan is reviewed before it is built, for its design and against the design tree, by a session that did not write it, whatever the size of the change; a change too small to be worth that is joined with related ones into one work, never run alone.
Why: A small change left unreviewed can break what matters, and the damage is not small (a judgment, with no case behind it). A fix of a few lines that ran the flow alone took an hour and more with the waiting; what answers that is joining small things into one work, not leaving out the review.

### principle a-question-is-a-choice
What is asked of a person is a question of yes or no, or a choice of one or more among options, with the answer its asker recommends. The answer is the option taken and nothing else: why, where the owner wants to say it, is said in the owner's words to the cell.
Why: A question in free words cannot be answered quickly, and words given with an answer, taken as part of it, could not be told from what the owner says to the cell.

### principle a-question-says-first-what-to-decide
A question an agent puts to the owner says in its first sentence what is to be decided and what it affects, uses no inner names, gives what follows from each option and a recommendation, and keeps its details behind a link; it is short, and is opened only where it is not understood.
Why: Otherwise the owner reads the whole story again each time; a question written well can be decided with nobody's help.

### principle who-asks-can-be-spoken-to
A cell that may ask the owner a question can be spoken to by the owner; a cell that asks nothing is not spoken to.
Why: The owner may ask back about a question, and only the cell that asked knows why: words to another cell reach one that does not know it. A cell that asks nothing has nothing to take up; the owner's word on a review goes to the cell that answers it, which fixes the point or has the owner rule.

### principle the-flow-gives-the-trees-rules
The rules of the design tree are wf-agents', the same for every project, and the flow gives them to every agent in the system prompt of every session, as the project's guide is: no cell is left to find them. The guide points to them as well, for the agents that work outside the flow.
Why: Every project's agents need the same rules, and said once in the flow they reach every cell, where a rule an agent must find by itself may go unread. An agent outside the flow is given nothing by the flow and has only the guide.

### principle an-agent-knows-only-its-task
An agent works from its task and the data it is given, and knows nothing of the flow around it: not the cell it runs as, not the other cells, not who reads what it writes. A cell's words name no other cell.
Why: Carrying what one cell writes to the next is the flow's work (Article 7). Words that tell an agent about the flow copy the flow, go stale when it changes, and invite the agent to do the flow's work.

### principle a-cell-knows-only-its-data
A cell knows nothing of other cells: it gets only the data it watches and reads and the checkout at those versions, and runs only on a version new to it of a datum it watches, never one it wrote. Nothing else runs a cell, none excepted, and a need this cannot meet is a design fault, mended there.
Why: Triggers apart from the data became a second graph to keep in step: empty data to wake cells, a datum and trigger per answering pair, versions sent on by their writer. Each met one case, yet the owner had to ask at three steps of one work. A cell reached by more than its data cannot be tested on it.

### principle one-dialogue-for-each-party
Each agent's cell that judges or doubts another cell's work has its own dialogue with that cell, as the owner has with each cell he speaks to: one datum only the two write, so each word is new to the other alone. An act of the engine reads no words and is no party: what runs it again answers it.
Why: One record for several parties woke every party at each answer, and a datum made for each answer doubled the data and the triggers of every pair, growing with each cell added.

### principle the-one-woken-owes-the-next-word
In an open dialogue the party woken owes the next word, and a judge's pass, which closes the dialogue, is written as its verdict alone. The engine sends back a return that plainly leaves the word owed.
Why: If every side answered whenever woken, a judge could never pass without a word, and a pass with a word would wake the maker for nothing (a judgment, with no case behind it). A side left owing with nothing to send it back stalls the work with nobody woken, as a red check answered in words alone did.
