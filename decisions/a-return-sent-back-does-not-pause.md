# A return sent back queues a matter for the architect and does not pause the work; three in a row do

## What is done now

Each return of an engineer's work that the engine sends back queues one matter for the architect of the work's
project, in the same write that ends the round: kind `set_straight`, detail `sent_back`, no work or round of its
own (nothing is raised by a round the work waits at). Its body holds `work`, `round`, `cell`, `problems` (each problem
as `rounds.rejected` keeps it, `{check, field, text}`) and `reason` (the same problems as lines for reading, one line
for each, with the check, the field and what is wrong, and no JSON). The work does not wait for it: the same cell runs
again in the same session, told what was wrong. The matter stays in the queue when the work ends; the cause of a return
is still to be looked for.

The graph says this by `rounds.sentBack.look`, and only a graph with an architect above it may (the compiler refuses
it otherwise). The architect's own work has none above it: its returns sent back queue nothing.

Three returns in a row at one cell (`rounds.sentBack.pauseAfter`, which the compiler requires to be at least 2) pause the
work there, as before. Where the graph has `look`:

- the reason starts with `returns were sent back` and says every problem of the last return, one line each, and that
  the architect is asked to look at why;
- the architect lets the work go on by `resume_work` at its round on a place to set straight, without the owner; the
  owner's Start lets it go on too (`db::architect_may_resume` knows the start of the reason; the engine's other pauses,
  the owner's pauses and the architect's own work's pause are not the architect's to lift);
- while the work is paused so, the matters of its returns are waited for: the view `architect_queue` says `waits` is
  `agent` for them, so that the architect takes them before the places to set straight and the plans that nothing waits
  for; once the work goes on they are not waited for again, and the view needs no stored mark to say so.

The matters of returns sent back do not keep the hold of a project's new starts to set it straight
(`hold_to_set_straight`): the engine queues one at each return of any running work, so that waiting for them would keep
the hold for as long as works run.

Nothing here counts returns sent back differently: what counts as in a row is what `failed::since_outside` already said.

## Why

The item (#293) is the owner's goal of removing all returns sent back: each cause is to be found once and mended, one by
one, and the owner reads only the conclusion. Before this, a return sent back left nothing for the architect to look at,
and the pause after three in a row only waited for the owner (the item's example: work 258 stood 15.5 minutes at its
pause, and nobody looked at why).

The owner's words at the merge gate, relayed by the assistant, were that it could be changed back (in translation, "it can
be changed back"): one return sent back no longer pauses the work; three in a row do; the reasons of returns sent back
still go to the architect's queue to be looked at afterwards; a problem to a line stays. These are the owner's words and
the only ones of the owner's in this file.

The rest is the design's, the assistant's and not the owner's:

- One matter for each return, queued with the round's end, because the architect then sees each one, and the merging of
  like returns (#257) can group them by check and field. A matter queued later, at the pause or at the next step, can be
  lost: the owner's Pause and Start between the third return and the next step start the count afresh.
- The queue puts the matters of a paused work with those a work waits on, and the matters of a work that goes on with the
  places to set straight the plan queued, behind the work that waits.
- The architect lets a paused work go on, as well as the owner: a pause nobody looked at is what work 258 stood in.

## Options not chosen

- Pause the work at every return sent back (built first, on this item's branch, and not approved by the owner at the merge
  gate). It makes sure the architect looks before the cell runs again, at the price of every work waiting at every return
  sent back. The assistant's reasons, which are not the owner's words, were that a large share of works would wait
  longer than before, against the target that an agent should not sit idle (Article 3 of the constitution).
- Queue the matter only when the third return pauses the work. Returns that never reach three in a row would reach the
  architect not at all, though the owner wants the reason of each looked at.
- At the pause, queue a second matter marked `urgent`. Three returns would be four matters, the last return's problems
  would be said twice, and `urgent` would stay after the work goes on, so that the matter still went before those that are
  waited for. That the work is waited for is worked out from the work's state instead.
- Queue the matter at the next step instead of with the round's end. See above: it can be lost.
- Send the matter as a report (`report`), to be looked at at the next planning. A report waits until
  `architect.gathered_most` of them have gathered, is not looked at one by one, and would start the planning more often.
- Let the owner alone lift the pause after three in a row. The owner's words do not say it; the item wanted the architect
  to let a pause go on, and a pause is the one case where a work waits for a look at once.

## Challenges considered and turned down

None yet.

## How it was decided

The owner's ruling at the merge gate of this item, relayed by the assistant, replaced the first design, which paused at
the first return sent back. The assistant chose the rest on the owner's behalf in the design of this work (#293, work 329),
which the architect and the owner review. A different choice by the owner replaces this file.
