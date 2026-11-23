# Sessions

When a line of a work goes on in its session of the agents' program, what a session starts with, how sessions are kept warm and compacted, and how a session is forked.

### decision each-line-begins-its-own-session
No line's session is forked from another line's, or from a session of another work: each line begins its own session.
- rejected: an agent proposed forking the other lines from R0's session once it had read the tree, as the design had it; the owner rejected it, because no fork was needed yet.
- rejected: an agent proposed forking the plan's reviewer from R0's session, as the old tree had it; the owner rejected it, because a line one level above its item cannot be forked from the session of a lower line.
- rejected: the owner proposed keeping a session that holds the whole tree, and forking it to pick the entries for each work that starts on the same tree; the owner rejected it, because a larger project's tree would overflow that session, and the research would lose the reading R0 leaves in its own.

### decision a-line-goes-on-in-its-session-under-another-account
A line goes on in its session after the account in use changes, the owner switching or the engine moving: a change of account starts no new session, and the next round writes the new account's cache.
- replaced: Starting a new session for each line whose rounds came to run under another account was on main; the owner changed it, because with every account's sessions in one directory a line can go on in its own session, at the cost of one cache write.

### decision no-act-begins-a-session-anew
The owner has no action that starts a line's session anew; the records of sessions started anew that way before are read as they were.
- replaced: An action for the owner to start a line's session anew, a button on the work's page and a command, was on main; the owner changed it, because its page was broken and the action was of no use.

### decision a-session-is-kept-warm-from-its-own-last-call
Each session of a line its work can come back to is pinged `sessions.warm_minutes` after its own last call, whoever holds the work, unless a round of its own line is under way.
- replaced: Leaving a waiting session to let its cache expire was on main; the owner changed it, because while the context is not very large, a ping costs a fortieth of the write an expired cache costs, and almost always pays for itself.
- replaced: Sending no ping while the engine held every work as it started, or while one work was paused, was on main; the owner changed it, because a hold stops the work from going on, and a cache left to go cold during the hold costs a whole write when the work goes on.
- replaced: Pinging a session only while none of its work's rounds was under way, a ping holding back every round of its work, was on main; the owner changed it, because a round on one line left the work's other sessions unpinged for its whole length and their caches expired.

### decision one-switch-keeps-sessions-warm
One switch says whether sessions are kept warm at all: the engine's own state, kept in the store, switched at once on the owner's page, and on until the owner switches it off.
- replaced: Settings of the installation, read when the engine starts, that kept sessions warm through a pause was on main; the owner changed it, because he turns off one switch over every ping to stop the spending at once, and a switch that waits for a restart cannot do that.

### decision a-session-is-compacted-at-the-installations-number
Every round's command line gives the agents' program `sessions.compact_at`, the installation's own number, as the context size to compact the line's session at; no session starts anew for its size.
- replaced: A limit lowered to a level worked out from the history of the line's own sessions was on main; the owner changed it, because judging by history is for watching the system's health, and the limit is the owner's number.
- rejected: an agent proposed starting a line's session anew earlier, since each call of a round reads its whole context from the cache; the owner rejected it, because a large session is normal, and how the program calls its tools within a round is not the system's concern.
- replaced: The engine starting a new session for a session past its size was on main; the owner changed it, because the line then went back to its loading cell and through many cells again, while a compacted session goes on in its own cell.
- rejected: an agent proposed keeping the number in the agents' program's own settings file; the owner rejected it, because there the engine could not see the number and the page could not show it.
- replaced: The number named `sessions.context_limit` was on main; the owner changed it, because the name no longer said that it is where the program compacts the session.

### decision a-compaction-reopens-the-session-under-its-id
When a line's session is compacted during a round, the engine records it as the session reopened, which runs no cell, and the session goes on under its id.
- replaced: The engine at once running again, in the new session, a round cut off by a reopened session was on main; the owner changed it, because the engine knows nothing about sessions: it picks the next cell to run, and the rest is left to the filling.

### decision the-projects-files-as-the-work-has-them
A session's system prompt is made as it starts, from the project's files as they are in the work's worktree, never through a link, and kept with the session; the program reads none of them itself.
- replaced: The program reading the worktree's guide by itself, besides what the engine gave it, was on main; the owner changed it, because the guide was then in the context twice.
- rejected: an agent proposed reading the guide and the constitution from main only, so that no work changes the rules of its own agents; the owner rejected it, because these files are protected and his to change, and when they are changed on a branch they are most likely meant to hold there.
- replaced: The guide as a datum every agent cell watched, so that a session open when it changed was told of the change, was on main; the owner changed it, because the guide is part of the system prompt and not a block, and as a block it brought a series of faults.

### decision the-form-is-kept-as-the-system-prompt-is
The form of return a session is given is made when the session starts, from the flow installed at that time, and kept with the session beside its system prompt, in the same way.
- replaced: Making the form anew for every round from the installed flow was on main; the owner changed it, because the form is part of the system prompt sent by another route, and the two must be kept alike.

### decision a-changed-system-prompt-or-form-begins-a-new-session
A line begins a new session at its next round, as for a change of model, when the system prompt or form a new session would begin with now differs from its session's, or cannot be made.
- replaced: Leaving a line on the system prompt and form its session began with, until something else began a new session, was on main; the owner changed it, because a work under way then ran every later round on replaced rules and form, and an old form cannot hand in a field made required since.
- rejected: an agent proposed sending the change to the open session as a message, or renewing only its form; the owner rejected it, because a resumed session keeps its first system prompt, old rules beside new, and a new form loses the whole cache (data: `research/system-prompt-on-resume.md`).
- rejected: an agent proposed an action of the owner's to begin the sessions of a work anew; the owner rejected it, because that action was taken out before, and it would have the owner click each work after each install.
- rejected: an agent proposed beginning anew, at a move to another version, each line whose two versions' outlines digest its form otherwise; the owner rejected it, because this check covers a move by what each session began with, and a second mechanism for it would drift from this one.
- rejected: an agent proposed beginning every line's session anew at every move; the owner rejected it, because that pays again for sessions whose form is unchanged (principle keep-what-was-paid-for).

### decision a-fork-keeps-the-sessions-tools
A fork of a round's session runs through the engine on the session's whole command line, including its system prompt, form of return and tools, with a hook refusing every tool but the return.
- rejected: an agent proposed forking sessions by hand, on a command line it typed itself without the session's form of return; the owner rejected it, because a fork must carry what its session began with: one such fork read nothing from the cache and cost $13.83.

### decision a-fork-begins-with-a-message-every-fork-of-the-round-shares
A fork is sent a fixed first message, the same for every fork of the round, and then its question, so that the first fork writes the session's history to the cache and later forks read it.
- rejected: an agent proposed making a base fork when the cache has let the session go, which later questions fork from; the owner rejected it, because it needs the base fork's session id kept and a judgement whether its cache is alive, one more state (data: `research/fork-cache-shared-history.md`).
- rejected: an agent proposed a keep-warm ping in the session itself before the fork; the owner rejected it, because it helps only a fork at the session's end, not one at an earlier round's end, and writes one more round into the real session (data: `research/fork-cache-shared-history.md`).

### decision a-round-takes-the-session-its-line-was-in-as-it-began
A round runs in the session its line was in as it began; one recorded later, even on opening an older store, is the next round's, so a flagged round of such a store is retried in its own session.
- replaced: Running a round under way as an older store opened, for a flag no session followed, in the session that flag began was on main; the owner changed it, because a round's session would then follow from when its first try was recorded, not where the round began: a case handled apart.
- rejected: an agent proposed no longer beginning a session on opening for a failed round of an older store that no session follows; the owner rejected it, because that store's works would then need handling by hand before each restart.
