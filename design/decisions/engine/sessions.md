# Sessions

When a line of a work goes on in its session of the agents' program and when it begins a new one.

### decision a-new-session-loads-again-through-its-line-s-datum
A new session holds nothing of what a line's loading cell read into it, so a new session of a line writes the line's own datum `<line>_session`, which the flow has only where a cell of that line watches it, and that cell runs again by the rule that runs any cell; of the engineer flow only the engineer's line has one, and a new session of another line only begins.
Not knowing this, an agent would give every line the datum, or one datum to all lines, so that each new session or compaction of the engineer's line ran every line's loading cell again for nothing, the deep research at its highest effort among them; or have the deep research watch its line's datum, running its whole round again where its first round reads the tree anyway; or give each cell one, running every cell of a line again.

### decision a-lost-session-keeps-its-id
A round finds its session by counting its line's new sessions recorded before its first try began, and the round that finds its session lost began that try before the loss is recorded, so a session the agents' program lost is begun again under the same id, as a compacted one goes on under it, and only a new session for another reason takes an id of its own.
Not knowing this, an agent would give a session begun again an id of its own, as every other new session has; the session the round that found it lost begins would then be another than the one the rounds after it count.

### decision no-new-session-when-the-cache-expired
Going on in a session after its prompt cache expired writes the session to the cache once, while a new session runs its loading cell again and reads again what its rounds had read, which costs no less (a judgment, not measured), so a line goes on in its session after any wait, and begins no new one for the cache.
Not knowing this, an agent would begin a new session after a wait longer than the cache's life, taking it for cheaper than writing a long session again, or offer it as a choice of each line, which no line took when it was offered; the line would lose what its rounds had read and pay to read it again.

### decision waiting-sessions-are-kept-warm
A round after the cache's life writes the whole session again, a write kept an hour costing forty reads on Opus and Sonnet 5.5, twenty on Haiku 5.5 and eighty on Fable 5.1, so while a work waits with no round under way, each session of a line it can come back to is pinged `sessions.warm_minutes` after its last call, as many times in one wait as cost no more than one write beyond a read.
Not knowing this, an agent would let a waiting session go cold and pay at the next round for writing all of it again, as a fork that missed its base wrote 73,000 of its 107,000 tokens again for $0.57 more where a ping cost $0.03; or ping every line of the work, those it has passed for good too, paying for nothing.

### decision a-hold-keeps-the-pings
A hold stops the work going on and not what the work has already paid for (principle keep-what-was-paid-for), so its sessions are pinged whoever holds it, the engine as it opened, the owner's pause or a question to the owner, but not under the wait every work shares, where a ping fails as its rounds do, nor under the engine's pause for the budget, a ping counting in the work's spend.
Not knowing this, an agent would have each hold stop the pings of the works it holds, as when no ping ever ran on the installation and a work waited eight hours at a cell with its session gone cold; or ping under the budget's pause, spending where the budget is the refusal to let that work spend more.

### decision keeping-warm-is-the-engines-own-switch
The owner switches on the page whether the sessions of waiting works are kept warm, and a switch holds as it is switched, so it is the engine's own state, kept with the installation as the pause of everything is and on until the owner switches it off, and no setting.
Not knowing this, an agent would make it a setting, or keep a setting beside the switch; read as the engine starts, the setting would leave the switch doing nothing until the engine started again, and one thing would stand in two places (principle one-thing-one-place).

### decision nothing-runs-beside-a-ping
A round begun beside a ping would be filled as its session stood before the ping, which may compact it, and a worktree let go of under a ping takes the directory its program runs in, so a ping is a job of its work, and no round of the work begins, nor is anything else done for it, until its pings have ended.
Not knowing this, an agent would run a ping beside the work's next round or its end, since a ping takes no slot; the round's message would miss a compaction the ping made, its cost would be counted without the ping's, and the end could take away the directory the ping's program runs in.

### decision a-session-is-compacted-at-the-installations-number
A session begun anew for its size holds nothing of what its line read and did but what its loading cell reads into it again, so every round's command line gives the agents' program the installation's `sessions.compact_at` as the context to compact at, the engine begins no new session for size, and `--autocompact` in `agents.args` is refused, the number standing in one place (principle one-thing-one-place).
Not knowing this, an agent would begin a new session past a size, as when work 20's engineer, its session replaced at 808,765 tokens, went on holding nothing of what it had read and done over its build; or give the number only to a session's first round, though the program keeps it for no session it resumes and otherwise compacts near a million tokens.

### decision a-prompt-too-long-begins-a-new-session
A session that holds a prompt too long for the model would be too long for every round after it, so a line whose round failed for that begins a new session, and the round is given again in it once the work is let on, while no line begins one for its size.
Not knowing this, an agent would take this for a new session for size, which decision a-session-is-compacted-at-the-installations-number turns down, and drop it; every later round of the line would then fail as that one did.

### decision the-compaction-number-is-not-lowered-by-history
Judging by history is for watching the system's health and for a guard that stops what its history does not explain (principle judged-by-own-history), and a session larger than its line's earlier ones is no fault, so no line drawn from the history of a line's rounds lowers `sessions.compact_at`.
Not knowing this, an agent would compact at the line its history gives, as work 11 replaced the session of `code_reviewer` at 103,764 tokens where the installation's number was 500,000; the session would lose what it read sooner, and every round before a compaction is closed to a fork (decision a-fork-begins-at-the-end-of-its-round).

### decision the-projects-files-as-the-work-has-them
A change of the project's guide, constitution or tree's rules made in a work counts in that work, the files being the owner's to change (principle protected-files-are-the-owners), so a session's system prompt is made as it begins from those files in the work's worktree, at the paths the project's declaration on main names and never through a symbolic link, kept with the session, and given to every round of it.
Not knowing this, an agent would read the files from main, so that a change the owner made on the branch never reached the work; or follow a link to a file the protection does not cover; or leave a missing file out, a session without its rules looking like one with them (principle nothing-passes-for-normal); or make the prompt anew each round as the worktree changes (principle the-system-prompt-holds-what-a-line-shares).

### decision the-form-is-kept-as-the-system-prompt-is
The form of return is the system prompt sent another way, one form on every call of a session (principle one-form-of-return-for-a-session), so it is made as the session begins from the flow installed then, kept beside the system prompt by the same means, and made anew only for a new session.
Not knowing this, an agent would make the form anew for each round from the flow installed, so that it changed within a session as soon as another flow was installed; or keep it by a means of its own beside the system prompt's, and the two ways would drift apart (principle one-thing-one-place).

### decision lines-say-where-sessions-begin
Lines say which cells share a session, so no cell begins a new session of its line at a place of its own, and a cell that must not go on in another's session is on a line of its own.
Not knowing this, an agent would mark a cell as beginning its line's session anew, a second way of saying which cells share a session, with a mark of its own on the chart (principle one-thing-one-place).

### decision a-compaction-reopens-the-session-under-its-id
A compacted session holds a summary where it held what its line read, and the program keeps it under its id, so each compaction during a round is recorded as the session reopened under the same id while the round goes on: the round's whole message is given again, and the line's datum, where it has one, gets a new version, but for a compaction within its loading cell's own round.
Not knowing this, an agent would let a compaction pass unrecorded, so that the next round was filled as though the session still held what it read; or record it under a new id, and the next round would begin a session that exists (decision a-lost-session-keeps-its-id); or run the loading cell again for its own compaction, which would load again without end.

### decision a-fork-keeps-the-sessions-tools
The cache is read only as far as a request begins as its session's did, and the tools stand at its front before the system prompt (principle one-form-of-return-for-a-session), so a fork is given the command line of its session's last round whole, its tools among it, and a hook refuses each use of a tool but the program's own tool for the return.
Not knowing this, an agent would take the tools from a fork, which is there only to answer, and the fork would read none of its session from the cache and pay for all of it again; or refuse the return's tool as well, leaving the fork no way to answer in the return's form.

### decision a-fork-begins-at-the-end-of-its-round
A fork asked of a round must hold only what that round saw, so where the session went on past it the fork begins at the round's last message, as the stream of its latest try that names one names it, and is refused where the session was compacted or begun again under its id after that round, the program then loading none of its earlier messages.
Not knowing this, an agent would begin the fork at the session's end, as a fork of work 11's first plan once held its later reviews, the owner's answers and the whole build, and could not show how that plan was written.

### decision a-forks-cost-is-counted-as-its-program-counts-it
The program counts a session it goes on with from all of it, whatever message a fork begins at, and the first call after a round wrote the cache on past that round, so a fork's spend is the program's total less all its session cost before, and the cache it reads is counted from that call where it came within the cache's life, else from the round's end, the latter a judgment, not measured.
Not knowing this, an agent would take from the total only what the session cost up to the round, as when a fork of a plan was told as $12.28 where it cost about $0.65; or count the cache from the round's end, and tell a fork its cache had let go sooner than it had.

### decision a-fork-of-an-ended-work-stands-in-for-its-worktree
The program finds a session by the directory it ran in, and an ended work's worktree is let go while its branch is kept (decision letting-go-at-the-end), so a fork of an ended work runs in a checkout of its branch, on no branch, at the worktree's place, taken away after, also when the fork is given up, the work held until then; a work not ended and with no worktree is refused.
Not knowing this, an agent would leave the checkout behind, where the next fork would take it for the worktree and a reopened work's start, which takes only a worktree on its branch, would stop; or take away the work's own worktree, racing its end; or make one for a work that goes on, whose next round would take it for its worktree.

### decision a-forks-spend-is-kept-with-its-answer
A fork is a look at one round's words by whoever plans the work, asked apart from the work's way through the flow, so its spend is said to whoever asked it and kept with its stream and its answer beside its round's, not in the work's records.
Not knowing this, an agent would record a fork's spend in the work's records as a round's or a ping's is, since it runs in the work's session; the work's spend, which its budget is held to, would then count looks taken at it, and its records would hold an event of no step of its flow.
