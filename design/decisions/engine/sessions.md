# Sessions

When a line of a work goes on in its session of the agents' program and when it begins a new one.

### decision a-new-session-loads-again-through-its-line-s-datum
So that a new session of a line is loaded again before the line goes on, the engine writes the line's own `session` datum, `<line>_session`, which only a cell that watches it runs on; on a line with no such cell a new session only begins.
Not knowing this, an agent would give every line the datum, or one to each cell: the loading cell of each other line, a round of the deep research among them, would run again for nothing at every new session or compaction, since a session is a line's and its cells share it.

### decision a-lost-session-keeps-its-id
So that the rounds of a line count their session rightly, a session the agents' program no longer has is begun again under the same id, and a compacted one goes on under it; only a new session for another reason takes an id of its own.
Not knowing this, an agent would give the new session an id of its own, and the round that found its session lost, whose first try began before the loss is recorded, would count another session than the rounds after it.

### decision no-new-session-when-the-cache-expired
So that a line is not given up for its cache, a line whose prompt cache expired while it waited goes on in its session, which writes the cache once.
Not knowing this, an agent would begin a new session, which runs the loading cell again and reads again what the rounds had read, costing no less than the one write (judgment, not measured).

### decision waiting-sessions-are-kept-warm
So that the next round of a work that waits does not write its whole session to the cache again, the session of each line it can come back to is sent a keep-warm ping `sessions.warm_minutes` after its last call, as often as its model's prices make worth it, taking no slot, its cost in the work's spend.
Not knowing this, an agent would ping without a bound, or a line the work has passed for good, or leave the cost out: a write kept an hour costs forty reads on Opus and Sonnet 5.5, twenty on Haiku 5.5, eighty on Fable 5.1.

### decision a-hold-does-not-stop-the-pings
So that a hold does not let go of what the work has paid for, its sessions are pinged whoever holds it (the engine as it opened, the owner's pause, a question to the owner); only the wait every work shares and the engine's pause for spending more than it may stop them.
Not knowing this, an agent would stop the pings under every hold (principle keep-what-was-paid-for), and a work waited eight hours at a cell with its session let go cold; a ping under either exception would fail as the rounds do, or spend where the budget refuses it.

### decision keeping-warm-is-the-engines-state
So that the owner's switch works at once, whether the sessions of works that wait are kept warm is the engine's own state, kept with the installation and switched on the owner's page, and no setting; they are kept warm until it is switched off.
Not knowing this, an agent would make it a setting: one read as the engine starts leaves the switch doing nothing until the engine starts again, and kept in both places one thing would stand in two (principle one-thing-one-place).

### decision no-round-begins-beside-a-ping
So that a round is filled as its session stands, no round of a work begins until its pings have ended, and what else is to be done for the work, as letting go of its worktree, waits for them.
Not knowing this, an agent would begin a round beside a ping: it would be filled as the session stood before the ping, which may compact it, with its cost counted without the ping's, and a worktree let go of would take the directory the ping's program runs in.

### decision a-session-is-compacted-at-the-installations-number
So that a line goes on in its session with a summary of it, every round's command line gives the program `sessions.compact_at` as the context to compact at, `--autocompact` in `agents.args` is refused, and the engine begins no new session for a session's size; a round whose prompt was too long for the model begins one.
Not knowing this, an agent would replace a session at some size: one replaced at 808,765 tokens held nothing of what the line had read and built. Left to itself the program compacts near a million tokens, and it keeps the number for no session it resumes.

### decision the-compaction-number-is-not-drawn-from-history
So that a session is not replaced for being larger than its line's earlier ones, `sessions.compact_at` is not lowered to what the history of the line's sessions gives.
Not knowing this, an agent would apply principle judged-by-own-history, which is for watching the system's health and for guards, and replace a session of 103,764 tokens because its line's was 81,768, where the number was 500,000: a larger session is no fault.

### decision the-projects-files-as-the-work-has-them
So that a change of the project's files (its guide, constitution and tree's rules) made in a work counts in that work, a session's system prompt is made as it begins from the work's worktree, by the paths the project's declaration names, and kept with the session; every round of it is given the kept one.
Not knowing this, an agent would read them from main, thinking it keeps a work from changing the rules of its own agents, which their protection does (principle protected-files-are-the-owners); or make the prompt anew each round, so that it changes within a session (principle the-system-prompt-holds-what-a-line-shares).

### decision a-missing-project-file-is-said
So that a session without its project's rules is not taken for one with them, the engine does not start where main lacks one of those files or holds one through a symbolic link, and a round whose worktree lacks one fails.
Not knowing this, an agent would leave a missing file out of the prompt (principle nothing-passes-for-normal), or read one through a link, which would carry into the prompt a file the protection does not cover.

### decision the-form-is-kept-as-the-system-prompt-is
So that a session is given one form of return on every call, the form is made as the session begins, from the flow installed then, and kept beside its system prompt by the same means; only a new session makes it anew.
Not knowing this, an agent would make it for each round from the flow installed, which changes within a session when another flow is installed (principle one-form-of-return-for-a-session), or keep it by a means of its own, which would drift from the prompt's (principle one-thing-one-place).

### decision lines-say-where-sessions-begin
So that lines are the one way to say which cells share a session, no cell begins a new session of its line at a place of its own; a cell that must not go on in another's session is on a line of its own.
Not knowing this, an agent would add a place within a line where its session begins anew, with a mark of its own on the chart, a second way of saying what lines say.

### decision a-compaction-reopens-the-session-under-its-id
So that a compacted session, which holds a summary where it held what the line read, counts as one reopened, each compaction during a round is recorded as the session reopened while the round is under way, and the session goes on under its id.
Not knowing this, an agent would give it a new id, and the next round would begin a session that exists (decision a-lost-session-keeps-its-id); or record nothing, and the cell that reads the tree into the line's session would not run again.

### decision a-fork-keeps-the-sessions-tools
So that a fork reads its session from the prompt cache, it is given the command line of the session's last round whole, its tools among it, and a hook refuses each use of a tool but the program's own for the return.
Not knowing this, an agent would take the tools away to make the fork safe: they stand at the front of every request, as the form does (principle one-form-of-return-for-a-session), so the fork would read none of its session from the cache, and a hook refusing the return's tool too would leave it no way to answer.

### decision a-fork-begins-at-the-end-of-its-round
So that a fork holds only what its round saw, where the session went on past the round it begins at that round's last message, as the stream of the round's latest try that names one names it, and is refused where the session was compacted or begun again under its id after the round.
Not knowing this, an agent would begin it at the session's end: a fork of a first plan held its later reviews and the whole build, and could not show how the plan was written. After a compaction the program loads no earlier message, so `sessions.compact_at` also decides how many rounds stay open to a fork.

### decision a-forks-cache-is-counted-from-the-call-after-its-round
So that the cache a fork reads is told rightly, it is counted from the first call after the round, a ping or the next round's start, where that came within the cache's life, else from the round's end.
Not knowing this, an agent would count from the session's last call: the first call after the round read what the round left and wrote the cache on past it, so no later call read that again (judgment, not measured).

### decision a-fork-of-an-ended-work-stands-in-for-its-worktree
So that the program finds the session of an ended work whose worktree is gone, a checkout of its kept branch, on no branch, is made at the worktree's place for a fork and taken away after, also when the fork is given up; a work that has not ended and has no worktree is refused.
Not knowing this, an agent would leave the checkout behind, which would stop a reopened work's start, or take away the work's own worktree, which is the end's to let go (decision letting-go-at-the-end); an empty directory would not show the program a repository (judgment, not measured).

### decision a-forks-spend-is-kept-with-its-answer
So that a fork's cost is told rightly, its spend is the program's total less all the session cost before, as the program counts a session it goes on with from all of it, and is said to whoever asked and kept with its stream and answer beside its round's, not in the work's records.
Not knowing this, an agent would take from the total only what the session cost up to the round, and a fork of a plan was told as $12.28 where it cost about $0.65; or put it in the work's records, though a fork is a look apart from the work's way through the flow.
