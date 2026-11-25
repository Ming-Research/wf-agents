# What the page reads from the engine

How the engine answers the reads of the owner's page, and when what it answers is worked out again.

### decision an-items-summary-is-kept-while-what-it-is-made-from-is-the-same
The engine keeps each item's summary and gives the page, on a cursor, only the items changed since its last read; an item is worked out again only when what its summary is made from changed.
- replaced: Working every item's summary out again at every read of the items was on main; the owner changed it, because a read took 0.75 to 1.1 s at 354 items and held every other read of the page behind it (data: `research/items-read-measured.md`).
- replaced: The page taking the whole list of items, 716 KB at 354 items, at each notice of a change was on main; the owner changed it, because the page then read and drew what had not changed (data: `research/items-read-measured.md`).

### decision a-work-under-way-is-looked-at-again-after-30-seconds
A work with a round or a call of a tool under way has its time bar worked out again and git's look at its worktree made again once 30 seconds have passed, in the read that finds them old and not before; the page asks at that rate while it is seen.
- replaced: Working out the time bar and looking at the worktree again at every read of a work under way was on main; the owner changed it, because 17 works under way made 17 looks of git at 0.05 to 0.13 s at each read (data: `research/items-read-measured.md`).
