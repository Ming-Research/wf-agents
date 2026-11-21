# The context manager

What a round of an agent's cell is sent as the cell is entered, and what its session holds.

### decision every-round-is-filled-with-what-its-session-lacks
Each round of an agent's cell, the first as any other, is sent the contents of the data its cell is given that its session lacks, each as the cell gives it, and what of them is new to the cell.
- replaced: A cell given its whole input each time it was entered was on main; the owner changed it, because cells are entered again and again, and carrying the whole input each time would soon fill the context.
- replaced: The compiler working out ahead whether an input was already in the session was on main; the owner changed it, because its guess was wrong both ways, and what a session holds is known only as it runs.
- replaced: Inputs given at different times by where they came from, written by the line, needed from the start or from outside, was on main; the owner changed it, because what a session lacks is given and what it holds is not, wherever it came from.
- replaced: A round told only the names and versions of its data, its agent reading the contents itself, was on main; the owner changed it, because a round is to be given the contents, not a list of what changed.
- rejected: an agent proposed a cell's first round given the names and versions alone, the contents read in later rounds; the owner rejected it, because the model has no first round: a cell's first round is only the engine finding it by its data.

### decision a-datum-is-given-in-one-of-three-kinds
A cell gives each datum it watches or reads as its latest, as the version it last worked from and the latest, or as all its versions; each is filled by its own kind, and the kind is the cell's.
- rejected: an agent proposed that where one datum of a round is of the first kind, the cell make everything anew and the others be given their latest; the owner rejected it, because each input is filled by its own kind, as a cell may take a dialogue whole and the plan as its change.
- rejected: the owner proposed giving nearly every cell only what changed in the data it works from; the owner rejected it, because the kinds give that already where a cell works from what changed, a dialogue whole and the rest as their latest.

### decision a-datum-the-session-holds-is-given-as-its-change
A datum given as the last and the latest is sent as what changed since the version the cell last worked from, where the session holds that version and the change is shorter; otherwise whole.
- rejected: an agent proposed giving both versions whole; the owner rejected it, because the cell's task asks what changed, less is sent, and the whole is in the records.

### decision why-you-are-run-tells-each-new-datum
A message ends with "Why you are run": each datum the cell watches that is new to it, by its name, the version it last worked from and the latest, with what the cell does for it, the first time too.
- rejected: an agent proposed what ran the cell first, with its versions, then the data and the task; the owner rejected it, because the task says what the cell does, and what ran it then says what changed and what to do for it, in that order.
- rejected: an agent proposed words of the engine's own for the design tree and the session, with general sentences in a cell's task; the owner rejected it, because words for named data would be copied into every cell.
- rejected: an agent proposed listing a datum the cell never dealt with without what it does for it; the owner rejected it, because a cell then did not know how far to deal with each.

### decision the-task-is-given-once-a-session-under-its-heading
A cell's task is given once a session, after the data, under `## Your task`; where the session holds it, the heading comes alone, with a line that it was given before.
- rejected: the owner proposed putting the tasks of a line's cells in its system prompt, each message only naming the cell; the owner rejected it, because every cell would read the others' tasks, and a changed task would wait for a new session.
- rejected: an agent proposed the task right after the heading, so that sessions of different works share a cached beginning; the owner rejected it, because what is sent stays in the session whatever its order, and the model does better reading the material before what it is to do.
- rejected: an agent proposed sending nothing of the task where the session holds it; the owner rejected it, because the cell would then not know what it is to do.
- rejected: an agent proposed dropping the heading `## Your task`, the cell's title naming the task; the owner rejected it, because the task buried among headings is easily misread.
- replaced: A return sent back given the task again in full was on main; the owner changed it, because the agent had done the work and only handed it in wrong, and is to be told only that.

### decision a-cell-is-told-its-own-blocks-once-a-session
A cell is told the blocks it writes, with their words, once a session with its task; the system prompt tells only how every cell returns, the same for the whole flow.
- rejected: an agent proposed a system prompt telling the line's whole set of fields, each cell told the part it may use; the owner rejected it, because the agent would be left to match the two.
- rejected: an agent proposed giving each cell its own form of return every time it is entered; the owner rejected it, because it is given once in a session, as any material is.
- replaced: The words on returning sent beside each cell's task, three times in a session of the engineer line, was on main; the owner changed it, because they belong to the line's system prompt, after the engine's own words.
- replaced: Every cell told in the system prompt how to ask the owner was on main; the owner changed it, because cells that may not ask were told they may, and a question from one of them sent the whole return back.

### decision a-version-is-told-by-the-line-that-wrote-it
Each version a round is given says the line of the cell whose run wrote it, or that the owner wrote it, worked out from the records and the flow and never stored.
- replaced: A round never told who wrote a version was on main; the owner changed it, because a reopened session could not tell a point from a reply, two cells of one party judged whose turn it was wrongly, and a judge could not see that the owner ruled.
- rejected: the owner proposed naming the cell that wrote it; the owner rejected it, because one party may be two cells, a cell is not to know the other cells, and cells are renamed.
- rejected: an agent proposed naming the writer by its part, as the judge, the maker or the doubter; the owner rejected it, because the parts need a mapping in the flow's files, and would mislabel what a cell wrote itself.

### decision a-compaction-gives-the-round-again-at-once
A session compacted during a round is given the round's message again, whole, by the program's hook as the compaction ends, and counts as reopened from there.
- replaced: A session compacted during a round given what it lost only at the next round was on main; the owner changed it, because the rest of the round would otherwise run without what it was given.

### decision the-work-is-told-once-a-session
Which work and item a round is of is a part of the message the session keeps: told once a session, and again only where it changes.
- rejected: an agent proposed a line under the heading of every message; the owner rejected it, because it never changes within a line.
- rejected: the owner proposed putting it in the line's system prompt; the owner rejected it, because the system prompt must be the same for every session of a line, and one character that differed lost the whole cache they shared.

### decision a-datum-with-no-version-is-left-out
A datum a cell is given that has no version yet is left out of the message: no line says it has none.
- rejected: an agent proposed a line for each, saying it has no version yet; the owner rejected it, because with nothing to read it takes no part.
- replaced: Inputs that held nothing sent all the same in every message was on main; the owner changed it, because nothing is to be sent of what holds nothing.
