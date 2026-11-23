# The context manager

What the engine sends to a round of an agent's cell when the cell is entered, and what the round's session holds.

### decision every-round-is-filled-with-what-its-session-lacks
Every round of an agent's cell, the first included, is sent the contents of the cell's data that its session lacks, each as the cell gives it, and which of them are new to the cell.
- replaced: Giving a cell its whole input each time it was entered was on main; the owner changed it, because cells are entered again and again, and sending the whole input each time would soon fill the context.
- replaced: The compiler working out in advance whether an input was already in the session was on main; the owner changed it, because its guess was wrong both ways, and what a session holds is known only while it runs.
- replaced: Giving inputs at different times by where they came from, whether written by the line, needed from the start or from outside, was on main; the owner changed it, because a session is given what it lacks and not what it holds, wherever that came from.
- replaced: Telling a round only the names and versions of its data, for its agent to read the contents itself, was on main; the owner changed it, because a round should be given the contents, not a list of what changed.
- rejected: an agent proposed giving a cell's first round only the names and versions, with the contents read in later rounds; the owner rejected it, because the model has no first round: a cell's first round is only the engine finding the cell by its data.

### decision a-datum-is-given-in-one-of-three-kinds
A cell gives each datum it watches or reads as its latest version, as the version it last worked from and the latest, or as all its versions; each is filled by its own kind, which the cell chooses.
- rejected: an agent proposed that where one datum of a round is of the first kind, the cell redo everything and the other data be given as their latest; the owner rejected it, because each input is filled by its own kind, so a cell may take a dialogue whole and the plan as what changed in it.
- rejected: the owner proposed giving nearly every cell only what changed in the data it works from; the owner rejected it, because the kinds already do that where a cell works from what changed, and give a dialogue whole and the rest as their latest.

### decision a-datum-the-session-holds-is-given-as-its-change
A datum given as the last and the latest is sent as what changed since the version the cell last worked from, if the session holds that version and the change is shorter; otherwise it is sent whole.
- rejected: an agent proposed sending both versions whole; the owner rejected it, because the cell's task asks what changed, less is sent, and the whole versions are in the records.

### decision the-entries-a-block-names-are-given-whole
A block marked `names_tree_entries` is given as each entry it names, whole, from the tree's latest version on the branch, by the project's `[tree_format]`; a line naming none is sent back.
- replaced: Each line's session reading the tree for itself, given only its files changed since then, was on main; the owner changed it, because the lines read different parts of it, and a review did not know the decisions the engineer worked from (data: `research/tree-reading-measured.md`).

### decision a-datum-a-move-renamed-is-given-again-whole
A session going on through a move is given whole each datum the move renamed, and each whose name a renumbering gave another cell's datum; what it was given before stays under its names then.
- rejected: an agent proposed counting what the session holds under a datum's old name as held under its new one; the owner rejected it, because the agent would be told it holds a name it never saw, and could only guess.
- rejected: an agent proposed telling the session each old name beside its new one; the owner rejected it, because that is a line of its own in every message, for what comes once a renumbering.

### decision why-you-are-run-tells-each-new-datum
A message ends with "Why you are run": for each new datum the cell watches, its name, the version last worked from, the versions that changed its meaning, and what the cell does, the first time too.
- replaced: Telling the version it last worked from and the latest was on main; the owner changed it, because when several versions come in at once the latest alone hid the one that changed the meaning, and the cell went by the latest.
- rejected: an agent proposed telling only the versions that changed the meaning; the owner rejected it, because a datum sent whole shows only its latest version in its heading, so the cell would no longer learn which version it last worked from, or that it never had.
- rejected: an agent proposed keeping the latest version beside the version last worked from and the versions that changed the meaning; the owner rejected it, because the cell went by the latest.
- rejected: an agent proposed telling the version the engine consumed, which is never larger than a version that changed the meaning; the owner rejected it, because the message and the page would then name different versions as the one the cell last worked from.
- rejected: an agent proposed putting what ran the cell first, with its versions, and then the data and the task; the owner rejected it, because the task says what the cell does, and then what ran it says what changed and what to do about it, in that order.
- rejected: an agent proposed words of the engine's own for the design tree and the session, with general sentences in a cell's task; the owner rejected it, because words for named data would then be copied into every cell.
- rejected: an agent proposed listing a datum the cell never dealt with, without saying what the cell does for it; the owner rejected it, because the cell then did not know how far to deal with each.

### decision the-task-is-given-once-a-session-under-its-heading
A cell's task is given once per session, after the data, under `## Your task`; if the session already holds it, the heading comes alone, with a line saying it was given before.
- rejected: the owner proposed putting the tasks of all a line's cells in its system prompt, with each message only naming the cell; the owner rejected it, because every cell would read the others' tasks, and a changed task would have to wait for a new session.
- rejected: an agent proposed putting the task right after the heading, so that sessions of different works share a cached start; the owner rejected it, because what is sent stays in the session in any order, and the model does better reading the material before what it has to do.
- rejected: an agent proposed sending nothing of the task if the session already holds it; the owner rejected it, because the cell would then not know what it has to do.
- rejected: an agent proposed dropping the heading `## Your task` and letting the cell's title name the task; the owner rejected it, because a task buried among headings is easily misread.
- replaced: Sending the task again in full when a return was sent back was on main; the owner changed it, because the agent had done the work and only handed it in wrongly, and should be told only that.

### decision a-cell-is-told-its-own-blocks-once-a-session
A cell is told the blocks it writes, each with the flow's words for it, once per session with its task; the system prompt says only how every cell returns, the same for the whole flow.
- rejected: an agent proposed a system prompt listing the line's whole set of fields, with each cell told the part it may use; the owner rejected it, because the agent would be left to match the two.
- rejected: an agent proposed giving each cell its own form of return every time it is entered; the owner rejected it, because it is given once per session, like any other material.
- replaced: Sending the words on how to return beside each cell's task, three times in a session of the engineer line, was on main; the owner changed it, because they belong in the line's system prompt, after the engine's own words.
- replaced: Telling every cell in the system prompt how to ask the owner was on main; the owner changed it, because cells that may not ask were told they may, and a question from one of them got the whole return sent back.

### decision a-version-is-told-by-the-line-that-wrote-it
Each version a round is given names the line of the cell whose run wrote it, or says the owner wrote it, as the records and the outline of the work's version say; it is never stored.
- replaced: Never telling a round who wrote a version was on main; the owner changed it, because a reopened session could not tell a point from a reply, two cells of one party misjudged whose turn it was, and a judge could not see that the owner had ruled.
- rejected: the owner proposed naming the cell that wrote it; the owner rejected it, because one party may be two cells, a cell should not know the other cells, and cells get renamed.
- rejected: an agent proposed naming the writer by its role, such as the judge, the maker or the doubter; the owner rejected it, because the roles need a mapping in the flow's files, and would mislabel what a cell wrote itself.
- replaced: Reading the line from the flow installed was on main; the owner changed it, because a run was then told by the line its cell has in a flow the work never ran on (principle the-past-as-it-ran).

### decision a-compaction-gives-the-round-again-at-once
When a session is compacted during a round, the program's hook gives it the round's whole message again as the compaction ends, and the session counts as reopened from there.
- replaced: Giving a session compacted during a round what it lost only at the next round was on main; the owner changed it, because the rest of the round would otherwise run without what it had been given.

### decision the-work-is-told-once-a-session
Which work and item a round belongs to is part of the message that the session keeps: it is told once per session, and again only when it changes.
- rejected: an agent proposed a line under the heading of every message; the owner rejected it, because it never changes within a line.
- rejected: the owner proposed putting it in the line's system prompt; the owner rejected it, because the system prompt must be the same for every session of a line, and one different character lost the whole cache they shared.

### decision a-datum-with-no-version-is-left-out
A datum a cell is given that has no version yet is left out of the message: no line says it has none.
- rejected: an agent proposed a line for each, saying it has no version yet; the owner rejected it, because with nothing to read it takes no part.
- replaced: Sending inputs that held nothing in every message anyway was on main; the owner changed it, because nothing should be sent for an input that holds nothing.

### decision the-situations-tool-derives-its-situations-by-rules
`wf-agents flow situations` derives a cell's situations from the flow by rules, which tests hold to the rounds of works of the repository's own flows.
- rejected: an agent proposed that the tool search, for each situation, a history that reaches it with the real context manager and write only those it reaches; the owner rejected it, because the tool stays fast, with no new mechanism and no change in what the crates depend on.

### decision a-try-keeps-what-it-was-given
Before its program starts, a try keeps what it runs on and what its message fills in a file beside it, and a later start records from there what the records lack.
- rejected: an agent proposed having each try wait, before its program starts, until the store has kept what it fills; the owner rejected it, because every start would then wait on the engine, and with the agents on another machine every try would wait across machines.
