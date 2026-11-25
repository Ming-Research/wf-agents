# What a read of the items costs on the installed engine

A record of what was read off the installed engine on one day, to say why the engine keeps each item's summary and gives
the page only what changed (principle `state-from-records`: a state worked out from the records is stored beside them
only where measured too slow). It is evidence of that choice and goes when the choice is next looked at; it states no
requirement.

## What was measured, and how

The installed engine on the owner's machine, on its loopback address, as it stood: 354 items, 261 of their works not
ended, 17 with a round under way, no change made to it. Each figure is `curl -w '%{time_total}'` on a `GET`, so the time
of the engine and of the loopback, no browser. Read on 2026-11-25.

| read | answer | time of 3 reads in turn |
|---|---|---|
| `GET /api/v1/items` | 716,409 bytes | 1.09 s, 0.87 s, 0.75 s |
| `GET /api/v1/attention` | 6,612 bytes | 0.24 s |
| `GET /api/v1/works/354` | 107,963 bytes | 0.10 s |
| `GET /api/v1/engine` | 69 bytes | 0.034 s, then 0.0016 s |

Where the 716 KB goes (bytes of the JSON of each field, summed over the 354 items): `work.cells` 346,405; `history`
82,688; `work.spent` 54,926; `work.pinged` 52,342; `work.forked` 51,752; `work.now` 32,347; `title` 27,201; `work.bar`
10,596. The other fields are each under 10,000.

One look git makes of a work's worktree, as the engine makes it for a work with a round under way (`git diff --numstat`
against main, then `git status --porcelain -uall`), in three of the worktrees on the machine: 0.01 to 0.08 s for the
first and about 0.05 s for the second, each.

## What it says

- The engine answers every read on its one loop, in order (`Message::Read`), so a read of the items that takes about a
  second holds every other read behind it, the cheap `GET /api/v1/engine` too: that read takes 2 ms by itself and the
  owner measured 2 s for it in the page.
- A read of the items works every summary out again: 354 of them, though a few change in a second. What is worked out
  again is the engine's own state read from records, which the page then also takes whole each time (716 KB), each time
  one of its notices says a work changed.
- A work with a round under way has git look at its worktree at every read, since a worktree under way may change at
  any moment (`crates/carry/src/changed.rs`, `Known::stat`): 17 looks of 0.05 to 0.13 s at every read.

## Not measured

How the second splits between the summaries and the git looks: the engine has no timing of either, and a figure taken
in the unoptimized build the checks run in is no figure of the installed engine. After the change a read works out only
the summaries whose inputs changed (`ItemsCache::built` counts them, and the route tests read it) and makes no git look
of its own, so the split matters only to the first read after the engine starts; it is read off the installed engine
once the change is installed, with the figures of a read with the cache warm.
