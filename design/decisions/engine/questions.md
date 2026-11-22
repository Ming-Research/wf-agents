# Questions and answers

What the engine keeps of the questions put to the owner and of his answers, and what the engine itself asks the owner.

### decision a-cell-that-asks-has-its-own-questions
Each cell that may ask has a datum of its own, `{cell}_questions`, which it writes and watches; the owner's answers are versions of it, and every agent cell is given those of every cell.
- replaced: One record of every cell's questions and answers for the work, `q_and_a`, with a block made for each asking cell to wake it, was on main; the owner changed it, because only the asking cell watches its own, so an answer wakes only that cell and no block is made just to wake a cell.
- rejected: an agent proposed hanging each answer on the version it answers, instead of making it a version of its own; the owner rejected it, because a cell given only the latest could then hold an answer without its question, and pairing the two would need a mechanism of its own.

### decision an-answer-that-changes-nothing-runs-no-cell
An answer is new to the cells that watch the questions only if it answers a question as last asked with an option other than the one the work goes by; any other answer is kept as changing nothing.
- rejected: an agent proposed that every answer run the asking cell again and be given to it; the owner rejected it, because picking the recommended option only records that he confirmed it, and changes no input.
- rejected: an agent proposed that an answer other than the recommended one reach the asking cell as the owner's words to it; the owner rejected it, because bound to the words it becomes a mess: words are words and questions are questions.
- replaced: A trigger of the asking cell, fired when another option was taken or the question held the work, was on main; the owner changed it, because the questions became a datum the cell watches, and a version that changes nothing saves a round run for nothing.

### decision the-merge-gate-is-a-datum-the-engine-writes
The engine writes `asked`: `waiting` while a question of the work has no answer from the owner to its latest version, and `answered` otherwise; the check before the merge requires `answered`.
- replaced: The check before the merge requiring only that no judge's verdict stood at fail was on main; the owner changed it, because a question not waited for let the work go on by its recommendation, and every question still open should have his answer before the merge.
- rejected: an agent proposed a second gate before the implementation, holding it while a question that stops the work is open; the owner rejected it, because any cell may ask, and a cell that needs an answer asks and waits by itself.

### decision a-question-says-whether-its-cell-waits
A question says whether its cell waits for the answer, which holds the work at that cell; one that does not lets the work go on by the option it recommends; a cell has no other way to stop for him.
- replaced: A confidence from one to five on each question, with four and five letting the work go on, was on main; the owner changed it, because four and five came down to stop or not anyway, and the agent can say itself whether to stop.
- rejected: the owner proposed that a cell that cannot go on say so separately, as a message with a blocking mark or through its output, answered by words that let it go on; the owner rejected it, because a question that stands alone and waits says the same, so no other kind is set up.
- rejected: an agent proposed a field on the question for why it waits, `why` and then `because`; the owner rejected it, because nobody reads it on its own: the reason goes into what the question asks.

### decision a-question-says-whether-its-answer-goes-into-the-tree
A question says in `tree` whether its answer settles a choice the tree's rules give the owner; if such an answer left the tree unchanged, the change is sent back and the check before the merge fails.
- replaced: Leaving it to the implementation to judge which answers settled a choice the tree's rules give the owner was on main; the owner changed it, because an answered choice could go unwritten with nothing to catch it, as a question of where his page shows an abnormal tool call showed.
- replaced: Looking only as a return hands in the change was on main; the owner changed it, because an answer that takes the recommended option to a question that does not wait runs no cell, so one given after the change was handed in could reach the merge unwritten.
- rejected: the owner proposed that the engine also send back a change of the tree that no question marked yes asked for; the owner rejected it, because mending an entry the change made untrue changes the tree with no question.

### decision a-question-is-written-under-its-name
A cell names each question it asks: written again under that name, it is asked again; given by its name with a reason, it is withdrawn; a question the cell does not write stands, with its answer.
- rejected: an agent proposed handing in all of a cell's questions again each time, with an unchanged one kept by its number; the owner rejected it, because a question left out was then withdrawn, and a rewritten one lost its answer.
- rejected: an agent proposed that the engine number each question and tell the cell the number in its next message; the owner rejected it, because nothing in the return that asks could then point to the question.
- rejected: an agent proposed withdrawing a question by its name alone; the owner rejected it, because the page could then show neither that it was withdrawn nor why.

### decision a-question-may-stand-alone
A question is an entry in its cell's questions, not in a text: a text the cell writes points to a question as `{{name}}` where they belong together, and one that no text points to stands alone.
- rejected: an agent proposed keeping each question in the text of the cell where it arises; the owner rejected it, because some cells cannot or should not change their text, and a changed text runs every cell that depends on it again.
- rejected: an agent proposed that a question name the block and the section it belongs to; the owner rejected it, because only a text embeds a question, and the text refers to the question anyway, so no reference both ways is needed.

### decision the-engines-questions-are-asked-in-the-cell-the-flow-names
What the engine itself asks of a work is asked in the questions of the cell its flow names (`engine_asks_in`), the implementation in the engineer flow, whatever round it is asked with.
- rejected: an agent proposed asking it in the questions of the cell whose round made what it asks about; the owner rejected it, because that routes by who wrote what, which the design has no place for.
- replaced: Keeping the engine's questions in the work's one record of questions and answers was on main; the owner changed it, because the implementation can change the work as the answer asks, and the owner can speak to it to ask why it was asked.

### decision the-check-holds-a-protected-change-until-the-owner-lets-it-merge
The engine asks the owner whether a work's change to protected paths may merge, recommending that it may: asked as a round ends it holds nothing, and the check before the merge passes only on his yes.
- replaced: Nothing stopping a change to the protected paths was on main; the owner changed it, because such a change should wait for his yes before the merge, as one more question waited for like the others.
- rejected: the owner proposed a cell of its own that checks a change to the protected paths; the owner rejected it, because the engine asking a question about it needs no cell of its own.
- replaced: The question asked when a round changed protected files, holding the work until the owner answered, was on main; the owner changed it, because the check holds to the change he let merge and a later change is asked about again, so a stop midway only made the work wait.
- rejected: an agent proposed that the question recommend no option, with the work going on as if the change may merge; the owner rejected it, because going on by that option before the answer is recommending it.

### decision a-protected-change-is-asked-with-the-work-s-reason-for-each-file
The engine asks about a protected change only once the work says why each file is needed; the question says the work changes files the project protects, then each file's path and reason over its diff.
- replaced: The question with only the diff was on main; the owner changed it, because where a change to a protected file is really needed, the agent should say why.
- replaced: A question that opened by asking whether the work's change to files that govern the agents may merge, with the full diff after it, was on main; the owner changed it, because he could not read it, and he knows what protected files are.
- rejected: an agent proposed a first sentence counting the files, a line for each, the diffs folded behind a link, and shorter options; the owner rejected it, because that was more than needed: the question should say that protected files changed, then the agent's reasons, then each change.
