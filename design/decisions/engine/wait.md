# Waiting

What a work waits on the owner for, and how whoever plans the work learns what needs them.

### decision nothing-is-pushed-to-the-planner
Whoever plans the work learns what needs them through `wf-agents wait`, a call that returns as soon as something does; nothing is pushed to them.
- rejected: an agent proposed that the board push a message to whoever plans the work as each item is done; the owner rejected it, because a message pushed while the assistant talks with the owner breaks into that talk, and that talk's context must stay on it.

### decision only-a-sign-in-waits-on-the-owner
When the agents' command line fails, only a cause no retry mends, like a refused sign-in or a program too old, waits on the owner, who is told what to do; the engine waits out any other cause itself.
- rejected: an agent proposed counting every case of the agents' program not starting as waiting on the owner; the owner rejected it, because only a sign-in or a refusal cannot be mended without the owner.
- rejected: an agent proposed counting a usage limit as waiting on the owner only where the engine cannot move; the owner rejected it, because the limit resets by itself, so the count would often point at what soon goes away, and this rule of only a cause no retry mends would change.
- rejected: an agent proposed counting every usage limit that holds the rounds as waiting on the owner; the owner rejected it, because every weekly limit would raise the count even where the engine has moved, and the count would no longer match what he can do.

### decision no-line-says-the-merge-waits
A question the owner is to answer shows only what it asks, and no line beside it says that the merge waits for it.
- rejected: an agent proposed the line "The merge waits for this answer." under a question's card; the owner rejected it, because the owner had called such a line needless, and the red card and the work's line at the check already say so.

### decision the-engine-reports-a-red-main-and-fixes-nothing
The engine reports that a project's main is red on tests, once for each time it came to be so, in `wf-agents wait` and on the System view; it makes no item to fix it and starts no agent.
- rejected: an agent proposed that the engine make an item to fix a red main and start an architect agent for it; the owner rejected it, because the architect session decides, by section 2 of `architect/rules.md`, whether to fix or to roll back, and the engine only states the fact.
