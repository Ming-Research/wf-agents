# Reads of the works

How the engine answers what the page and the command line read of the works.

### decision what-holds-every-work-is-worked-out-once-for-a-read
The driver's readings of one work (its view, what holds back its next round, what it waits on the owner for) take what
holds every work at once (the pause of everything, the waits of the services, the works taking a slot) from their
caller, which works it out once for its read: the read of the items, the lines of what waits for the owner, the System
view and a work's own page. The forms that read one work alone work it out each time.
Why: The waits and the slots taken are each worked out from every work's records, so worked out for each work a read
of many works costs their square: with 337 works a read of the items took 1.24 s, about nine tenths of it there, and
0.04 to 0.1 s once worked out once. Keeping it beside the records, worked out again as they change, was turned down:
worked out once for a read it costs little (principle state-from-records).

### decision what-a-work-changed-is-worked-out-off-the-loop
What a work changed, its summary's `changed`, each file of its changes and one file's diff, is worked out with git in
the project's repository off the loop, as the page reads it: what a work at a head changed once for each head, and
the head of a work whose worktree is gone once until it has one again, both kept in memory for the process; what a
worktree holds each time it is read, since its agent changes it. Keeping the numbers in the work's records was turned
down: git works them out from the head the engine keeps (decision each-head-is-kept-under-a-reference-of-the-engines),
and a read of every work after the engine starts again works out each once.
Why: Measured on wf-agents' repository, a head takes about 50 ms of git (where its branch left main and a count of
the lines) and a worktree about 55 ms (its branch's head, a diff, and a listing of what git does not track, each file
of which is read), most of it each command starting; worked out at every read, a list of 350 works would wait about 17
seconds. A read of every work asks of eight at once, so that, past the first after a start, it waits only on the works
with a worktree, a few tens of milliseconds.

### decision where-a-branch-left-main-holds-after-its-merge
Where a work's branch left main is the latest commit of main its head holds, and, of a head main holds already as one
a merge commit merged, the latest commit of main it held before that merge, the first parent of main's first commit
that holds it.
Why: The latest commit of main the head holds is the head itself once a merge commit has merged it, and what the work
changed would read as nothing; read so, it is the same before the merge and after, and a head on main's own line is
its own.
