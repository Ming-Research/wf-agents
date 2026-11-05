# A return sent back queues a report for the architect and does not pause the work; three in a row do

## What is done now

Each return of an engineer's work that the engine sends back queues one report for the architect of the work's
project, in the same write that ends the round: kind `report`, detail `sent_back`, and no work or round of its own as
its source, so that the architect's answer is told to no running work. Its body holds `work`, `round`, `cell`,
`problems` (each problem as `rounds.rejected` keeps it, `{check, field, text}`) and `reason` (the same problems as lines
for reading, one line for each, with the check, the field and what is wrong, and no JSON). The work does not wait for it:
the same cell runs again in the same session, told what was wrong. The report stays in the queue when the work ends,
since the cause of a return is still to be looked for. The architect handles it as it handles any report: finds what it
really is, merges it into an open observation that records the same cause, and otherwise records a new observation or
mends the cause by an item.

The graph says this by `rounds.sentBack.look`, and only a graph with an architect above it may (the compiler refuses
it otherwise). The architect's own work has none above it: its returns sent back queue nothing.

Three returns in a row at one cell (`rounds.sentBack.pauseAfter`, which the compiler requires to be at least 2) pause the
work there. Where the graph has `look`:

- the reason starts with `returns were sent back` and says every problem of the last return, one line each, and that
  the architect is asked to look at why;
- the architect lets the work go on by `resume_work` at its round on a report of a return sent back, without the owner,
  and only once it found why the returns were sent back and trusts the cell to hand in a return that passes the
  checks; where the fault is in a check or in a schema itself, it leaves the work paused and asks the owner. The
  owner's Start lets the work go on too. `db::architect_may_resume` knows the start of the reason; the engine's other
  pauses, the owner's pauses and the pause of the architect's own work are not the architect's to lift;
- while the work is paused so, the reports of its returns are waited for: the view `architect_queue` says `waits` is
  `agent` for them, so that the architect takes them before the reports and plans' steps that nothing waits for; once the
  work goes on they are not waited for again, and the view needs no stored mark to say so.

What counts as in a row is what `failed::since_outside` already said; nothing here counts returns sent back differently.

## Why

Removing every return sent back needs each cause found once and mended (Article 4): an anomaly is recorded and
summarized, so that it shows where the system needs to change. Before this, a return sent back left nothing for the
architect to look at, and the pause after three in a row only waited for the owner's Start, with its reason as the
column's JSON (Article 8: the page holds what the owner uses, not internal values).

The owner ruled, at the merge gate of the first design of this item, that a work must not pause at every return sent
back, and that the first design was to be changed back. The reasons below are the design's, reasoned from the
constitution, and are not the owner's:

- A work that waits at every return sent back makes agents sit idle for what the architect may look at afterwards
  (Article 3). A look after the fact loses nothing, since the problems are kept on the round and in the report.
- One report for each return, queued with the round's end, because the architect then sees each one (Article 2), and the
  merging of like returns (#257) can group them by check and field. A report queued later, at the pause or at the next
  step, can be lost: the owner's Pause and Start between the third return and the next step start the count afresh.
- The kind is `report`, not `set_straight`. A place to set straight is a step of the plan, which the architect takes as
  the plan judged it, and the next plan withdraws the steps before that no session took: a return sent back queued as
  one would be withdrawn before anyone looked at its cause, and would be handled as a step to take, not a cause to find.
  A report is handled by finding what it is and recording it (an observation, merged into the same one), which is what
  a return sent back needs; each report starts the architect's work when none runs, so each is looked at one by one.
- That the work is waited for is worked out from the work's state, in the view, so that the report needs no flag that
  could be left over after the work goes on (Article 7: the engine keeps what the chart shows, and does nothing beside
  it).
- The architect lets a pause go on, as well as the owner: a pause nobody looked at is the case that most needs a look,
  and a work waiting for the owner's Start alone waits as long as the owner is away (Article 3). It lets it go on only
  once it found why: letting a work go on whose return fails the same check again would pause it again after three
  more returns sent back.

## Options not chosen

- Pause the work at every return sent back, for the architect to look at before the cell runs again. The work then
  waits for a look at each, and the architect's rounds are on its critical path; the owner did not approve it.
- Queue the report only when the third return pauses the work. Returns that never reach three in a row would reach the
  architect not at all, though each cause is to be found.
- At the pause, queue a second report marked `urgent`. Three returns would be four reports, the last return's problems
  would be said twice, and `urgent` would stay after the work goes on, so that the report went before those that are
  waited for.
- Queue the report at the next step instead of with the round's end. See above: it can be lost.
- Queue it as a place to set straight (`set_straight`). See above: the next plan withdraws it, and its handling is
  that of a step of the plan.
- Let the owner alone lift the pause after three in a row. A pause is the one case where a work waits for a look at once,
  and the owner's page is not where a cause is found.

## Challenges considered and turned down

None yet.
