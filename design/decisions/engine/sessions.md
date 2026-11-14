# Sessions

When a line of a work goes on in its session of the agents' program and when it begins a new one.

### decision a-new-session-loads-again-through-the-block
A new session of a line that has a cell reading the block the outside source `session` writes writes that block, so that the cell runs again before the line goes on; a new session of a line with no such cell only begins. One block serves while one line has such a cell.
Why: That cell's whole job is to read into the session what the line's later cells work from, and a new session holds nothing of it; written as the outside, the block makes the cell run by the one rule that runs any cell, and its run changing nothing it wrote, nothing after it runs again. A line whose cells each read what they need from their message loses nothing a block would bring back.

### decision a-lost-session-keeps-its-id
A session the agents' program no longer has is begun again under the same id, and a compacted one goes on under it; only a new session for another reason takes an id of its own.
Why: Which session a round runs in is counted from the new sessions recorded before its first try began, and the round that finds its session lost began its first try before that is recorded: under an id of its own, the session that round begins would be another than the one the rounds after it count.

### decision no-new-session-when-the-cache-expired
A line does not begin a new session because its prompt cache expired while it waited.
Why: Going on writes the session to the cache once, while a new session runs the loading cell again and reads again what its rounds had read, which costs no less (judgment, not measured). Offered as a choice of each line, it was taken by none.

### decision the-projects-files-from-the-worktree
A session's system prompt is made as the session begins, with the project's files (its guide, its constitution, its tree's rules) read from the work's worktree by the paths `project.toml` beside the flows names, never through a symbolic link, and kept with the session in the data directory; every round of the session, however it begins in it, is given the kept one. The engine does not start where main, as the host last gave it, lacks one of the files or holds one through a link, and a round whose worktree lacks one fails.
Why: The owner ruled that a change of these files made in a work counts in that work: they are the owner's to change (principle protected-files-are-the-owners), and a change of them on a work's branch is meant to hold there. What keeps a work from changing the rules that govern its own agents is that the files are protected, not where the prompt is read from; read from main instead, a change the owner made on the branch would not reach the work. A link is refused because it would carry into the prompt a file the protection does not cover. The prompt is kept, since the worktree changes within a session, by the work's own commits and by main taken in, and a session keeps its system prompt as it began (principle the-system-prompt-holds-what-a-line-shares). A file missing is said rather than left out, since a session without its project's rules would look like one with them (principle nothing-passes-for-normal).

### decision lines-say-where-sessions-begin
No cell begins a new session of its line at a place of its own: a cell that must not go on in another's session is on a line of its own.
Why: Lines say which cells share a session; a place within a line where its session began anew was turned down as a second way of saying the same thing, with a mark of its own on the chart.

### decision a-compaction-reopens-the-session-under-its-id
A compaction of a line's session by the agents' program during a round is recorded as the session reopened, once for each compaction, while the round is under way, and the session goes on under its id.
Why: A compacted session holds a summary where it held what the line read, so the cell that reads into it must run again, and the rule that runs any cell, a new version of its input (document data-and-context, 25), does that only if the compaction is a new version of the data `session`. The program keeps the session under its id, so a new id would have the next round begin a session that exists (decision a-lost-session-keeps-its-id).
