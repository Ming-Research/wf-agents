# Waiting

What a work waits on the owner for, and how whoever plans the work learns what needs them.

### decision nothing-is-pushed-to-the-planner
Whoever plans the work learns what needs them through `wf-agents wait`, a call that returns as soon as something does; nothing is pushed to them.
- rejected: an agent proposed that the board push a message to whoever plans the work as each item is done; the owner rejected it, because a message pushed while the assistant talks with the owner breaks into that talk, and that talk's context must stay on it.

### decision only-a-sign-in-waits-on-the-owner
When the agents' command line fails, only a cause no retry mends, like a refused sign-in or a program too old, waits on the owner, who is told what to do; the engine waits out any other cause itself.
- rejected: an agent proposed counting every case of the agents' program not starting as waiting on the owner; the owner rejected it, because only a sign-in or a refusal cannot be mended without the owner.

### decision no-line-says-the-merge-waits
A question the owner is to answer shows only what it asks, and no line beside it says that the merge waits for it.
- rejected: an agent proposed the line "The merge waits for this answer." under a question's card; the owner rejected it, because the owner had called such a line needless, and the red card and the work's line at the check already say so.
