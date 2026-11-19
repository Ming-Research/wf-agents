# Sessions

When a line of a work goes on in its session of the agents' program and when it begins a new one.

### decision a-new-session-is-told-to-its-line-alone
A new session of a line is written as a datum of that line alone, present only where a cell of the line watches it, so that only that cell runs again before the line goes on; on a line with no such cell a new session only begins.
Not knowing this, an agent would give every line one shared datum, or each cell its own, and the loading cell of every other line would run again for nothing at each new session or compaction of the engineer's line; a session is a line's, and its cells share it.

### decision a-lost-session-keeps-its-id
A session the agents' program no longer has is begun again under the same id, and only a new session for another reason takes an id of its own, so that a round finds its session by counting the new sessions with ids of their own recorded before its first try began.
Not knowing this, an agent would give the session replacing a lost one an id of its own; the round that finds it lost began before that is recorded, so the session it begins would not be the one the rounds after it count.

### decision a-compaction-reopens-the-session-under-its-id
A compaction of a line's session during a round is recorded as the session reopened, once for each compaction, and the session goes on under its id, so that the cell that reads into it what the line works from runs again; the compaction during that cell's own round does not make it run again.
Not knowing this, an agent would give a compacted session a new id, so that the next round would begin a session that exists, or let the loading cell's own compaction run it again, and it would load again without end.

### decision no-new-session-when-the-cache-expired
A line does not begin a new session because its prompt cache expired while it waited, so a round after the cache's life goes on in the session, writing it to the cache once.
Not knowing this, an agent would begin a new session to avoid the write, which runs the loading cell again and reads again what the rounds had read, costing no less (judgment, not measured).

### decision a-session-is-compacted-at-the-installations-number
Every round's command line gives the agents' program `sessions.compact_at`, the installation's own number, as the context at which it compacts the line's session, and `--autocompact` in `agents.args` is refused, so that the engine begins no new session for a session's size.
Not knowing this, an agent would replace a session once it grew large, or lower the number from the line's history, and the new session would hold none of what the line had read and done: work 20's engineer, replaced at 808,765 tokens, went on without its whole build.

### decision a-round-that-fails-for-its-context-is-given-again-anew
A line whose round the service's safety check flagged, the run guard stopped, or whose prompt was too long for the model begins a new session, in which that round is given again once the work is let on.
Not knowing this, an agent would give the round again in the same session, which holds the same context: a prompt too long would be too long for every round after it, and a flagged or stopped round would be flagged or stopped again.

### decision the-projects-files-are-read-from-the-works-worktree
A session's system prompt is made as the session begins, with the project's files read from the work's worktree by the paths its declaration on main names, and kept with the session, every round of it given the kept one, so that a change of the files in a work's branch counts in that work.
Not knowing this, an agent would read them from main, taking the place they are read from to be what keeps a work from changing its own rules, where it is their protection (principle protected-files-are-the-owners); a change the owner made on the branch would not reach the work.

### decision a-missing-file-or-a-link-is-refused
The engine does not start where main, as the host last gave it, lacks one of the files the system prompt is made from or holds one through a symbolic link, and a round whose worktree lacks one of them fails.
Not knowing this, an agent would leave a missing file out or read through a link: a session without its project's rules would look like one with them (principle nothing-passes-for-normal), and a link carries into the prompt a file the protection does not cover.

### decision the-form-is-kept-as-the-system-prompt-is
The form of return a session's program is given with every call is made as the session begins, from the flow installed then, and kept with the session by the same means as its system prompt, so that only a new session makes one anew.
Not knowing this, an agent would make the form anew for each round, so that it changes within a session as soon as another flow is installed (principle one-form-of-return-for-a-session), or keep it by a means of its own, which would drift from the prompt's (principle one-thing-one-place).

### decision lines-say-where-sessions-begin
No cell begins a new session of its line at a place of its own: a cell that must not go on in another's session is on a line of its own, since lines say which cells share a session.
Not knowing this, an agent would give a cell a mark that begins a new session at it, a second way of saying what lines say, with a mark of its own on the chart (principle one-thing-one-place).

### decision waiting-sessions-are-kept-warm
While a work that has not ended waits with no round under way, each session of the lines it can come back to is sent a keep-warm ping `sessions.warm_minutes` after its last call, as many times in one wait as its model's prices make worth it, so that the next round reads the cache; a ping counts in the work's spend.
Not knowing this, an agent would leave the session to its cache's end, and the next round would write all of it again: a write kept an hour costs forty reads on Sonnet 5.5 where a ping costs one; or ping a line the work has passed for good, for nothing.

### decision a-hold-does-not-stop-the-pings
A ping is sent whoever holds the work (the engine as it opened, the owner's pause, a question the work waits on the owner for), but not while the wait every work shares holds the rounds, nor for a work the engine paused for spending more than it may.
Not knowing this, an agent would stop the pings under every hold, as it once did: no ping ever ran and a work waited eight hours with its session cold (principle keep-what-was-paid-for); or it would ping under the budget pause, spending where the budget refuses it, or under the shared wait, where a ping fails as the rounds do.

### decision keeping-warm-is-the-engines-state
Whether the sessions of works that wait are kept warm at all is the engine's own state, kept with the installation and switched on the owner's page, and no setting, so that they are kept warm until the owner switches it off.
Not knowing this, an agent would make it a setting read as the engine starts, which leaves the switch doing nothing until the engine starts again, and puts one thing in two places (principle one-thing-one-place).

### decision nothing-runs-beside-a-ping
Nothing else of a work runs beside its pings: no round of it begins until they have ended, and what else is to be done for it, as letting go of its worktree, waits for them.
Not knowing this, an agent would begin a round beside a ping, which would be filled as its session stood before the ping, which may compact it, with its cost counted without the ping's; and a worktree let go of under a ping takes the directory the ping's program runs in.

### decision a-fork-keeps-the-sessions-tools
A fork of a round's session is given the command line of the session's last round whole, its tools among it, with a hook that refuses each use of a tool but the program's own for the return, so that the fork reads its session from the cache.
Not knowing this, an agent would take the tools away to make the fork read-only: tools stand at the front of every request, so a fork given none reads none of its session from the cache and pays for all of it again; and refusing the return's tool too leaves it no way to answer in the return's form.

### decision a-fork-begins-at-the-end-of-its-round
Where the session went on past the round asked of, the fork begins at that round's last message, as the stream of the round's latest try that names one names it, and is refused where the session was compacted or begun again under its id after that round.
Not knowing this, an agent would fork at the session's end, and the fork would hold the later reviews, the owner's answers and the build, and could not show how that round's plan was written; after a compaction or a loss the program holds none of the earlier messages.

### decision a-fork-cache-is-counted-from-the-call-after-its-round
The cache a fork read is counted from the first call after the round asked of, a ping or the next round's start, where that came within the cache's life, else from the round's end.
Not knowing this, an agent would count it from the round's end and tell of a lapsed cache where the call after the round had written the cache on past it, so that no later call read the round's end again (judgment, not measured).

### decision a-forks-spend-is-the-total-less-what-came-before
A fork's spend is the program's total less all the session cost before the fork, so that it is right at whatever message the fork begins, the program counting a session it goes on with from all of it.
Not knowing this, an agent would take from the total only what the session cost up to the round asked of: a fork of a plan was told as $12.28 where it cost about $0.65.

### decision a-forks-spend-is-kept-with-its-answer
A fork's spend is said to whoever asked it and kept with what it was told, its stream and its answer beside its round's, and is not put in the work's records.
Not knowing this, an agent would count the fork's cost into the work's spend, though a fork is a look at one round's words asked apart from the work's own way through the flow, and the work would be held to a budget the look spent.

### decision a-fork-of-an-ended-work-stands-in-for-its-worktree
Where a work has ended and its worktree is gone, a fork of one of its sessions runs in a checkout of the work's kept branch, on no branch, made at the worktree's place and taken away after the fork, also when it is given up, the work held until then; a work not ended with no worktree is refused.
Not knowing this, an agent would leave the checkout behind, which the next fork would take for the worktree and which would stop a reopened work's start, or take away the work's own worktree, which the end's letting go owns (decision letting-go-at-the-end), or fork in an empty directory.
