# Sessions

When a line of a work goes on in its session of the agents' program, what a session begins with, how its waiting sessions are kept warm and compacted, and how one is forked.

### decision a-new-session-loads-again-through-its-line-s-datum
A new session of a line writes that line's own `session` datum, which the engineer's line alone has, R0 watching it, so that R0 reads the tree into the session before the line goes on.
- replaced: One `session` datum for the whole work, not one for each line, stood on main; the owner changed it, because every line's session reopens on its own, and the cells of another line were not told theirs was new.
- replaced: The deep research running again on every new session of its line stood on main; the owner changed it, because R0 reads the tree again and the cells after it go on in that session without each reading from the start.

### decision each-line-begins-its-own-session
No line's session is forked from another line's: each line begins its own session and reads the tree there for itself.
- rejected: an agent proposed forking the other lines from R0's session once it had read the tree, as the design had it; the owner rejected it, because no fork was needed yet.
- rejected: an agent proposed forking the plan's reviewer from R0's session, as the old tree had it; the owner rejected it, because a line a level above its item cannot be forked from a lower line's session.

### decision a-line-goes-on-in-its-session-under-another-account
A line goes on in its session after the owner takes another account: no session begins anew for an account, and the line's next round writes the session to the new account's cache.
- replaced: A new session for each line whose rounds came to run under another account stood on main; the owner changed it, because with every account's sessions in one directory a line can go on in its own, at the cost of one write of the cache.

### decision no-act-begins-a-session-anew
The owner has no act that begins a line's session anew; the records of sessions begun so before are read as they were.
- replaced: The owner's act to begin a line's session anew, a button on the work's page and a command, stood on main; the owner changed it, because its page was broken and the act of no use.

### decision waiting-sessions-are-kept-warm
While a work that has not ended waits with no round of it under way, each session of a line it can come back to is pinged `sessions.warm_minutes` after its last call, whoever holds the work.
- replaced: A waiting session left to let its cache expire stood on main; the owner changed it, because while the context is not very large a ping, a fortieth of the write an expired cache costs, almost always pays.
- replaced: No ping while the engine held every work as it opened, or while one work was paused, stood on main; the owner changed it, because a hold is on the work going on, and a cache let go cold under it costs a whole write when the work goes on.

### decision one-switch-keeps-waiting-sessions-warm
Whether waiting sessions are kept warm at all is one switch, the engine's own state kept in the store and switched at once on the owner's page, on until the owner switches it off.
- replaced: Settings of the installation that kept sessions warm through a pause, read as the engine starts, stood on main; the owner changed it, because one switch over every ping is turned off to stop spending at once, which a switch that waits for a restart cannot do.

### decision a-session-is-compacted-at-the-installations-number
Every round's command line gives the agents' program `sessions.compact_at`, the installation's own number, as the context to compact the line's session at, and no session begins anew for its size.
- replaced: A limit lowered to a line drawn from the history of the line's own sessions stood on main; the owner changed it, because judging by history is for watching the system's health, and the limit is the owner's number.
- rejected: an agent proposed renewing a line's session earlier, as each call of a round reads its whole context from the cache; the owner rejected it, because a large session is normal and how the program calls its tools in a round is not the system's.
- replaced: A new session begun by the engine for a session past its size stood on main; the owner changed it, because the line then went back to its loading cell and through many cells again, where compacted it goes on in its own cell.
- rejected: an agent proposed keeping the number in the agents' program's own settings file; the owner rejected it, because there the engine could not see the number nor the page show it.
- replaced: The number named `sessions.context_limit` stood on main; the owner changed it, because the name no longer said that it is where the program compacts the session.

### decision a-compaction-reopens-the-session-under-its-id
A compaction of a line's session during a round is recorded as the session reopened, which writes the line's `session` datum where it has one, and the session goes on under its id.
- replaced: The engine giving a round cut by a reopened session again at once in the new session stood on main; the owner changed it, because the engine knows nothing of sessions: it picks the next cell to run, and the rest is the filling's.

### decision the-projects-files-as-the-work-has-them
A session's system prompt is made as it begins from the project's files as the work's worktree has them, never through a link, and kept with the session; the program reads none of them itself.
- replaced: The program reading the worktree's guide by itself, beside what the engine gave it, stood on main; the owner changed it, because the guide then stood twice in the context.
- rejected: an agent proposed reading the guide and the constitution from main alone, so that no work changes the rules of its own agents; the owner rejected it, because these files are protected and his to change, and changed on a branch they are most likely meant to hold there.
- replaced: The guide as a datum every agent cell watched, so that a session open when it changed was told the change, stood on main; the owner changed it, because the guide is part of the system prompt and no block, and as one it brought a train of faults.

### decision the-form-is-kept-as-the-system-prompt-is
The form of return a session is given is made as the session begins, from the flow installed then, and kept with the session beside its system prompt, by the same means.
- replaced: A form made anew for every round from the flow installed stood on main; the owner changed it, because the form is part of the system prompt sent another way, and the two must be kept alike.

### decision a-fork-keeps-the-sessions-tools
A fork of a round's session runs through the engine on that session's command line whole, its system prompt, form of return and tools among it, a hook refusing every tool but the return.
- rejected: an agent proposed forking sessions by hand, on a command line it typed itself without the session's form of return; the owner rejected it, because a fork must carry what its session began with: one such fork read nothing from the cache and cost $13.83.
