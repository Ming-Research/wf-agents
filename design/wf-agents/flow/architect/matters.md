- The architect's flow is a loop: every step that may ask the owner is dispatched as a matter in the queue, wanting to
  check the policy again is putting a matter in the queue, and no line jumps there from another cell. A step that may
  ask must be a matter, so that there is something to hang when asking. A straight line into the loop, through reading,
  checking the policy and planning, had nothing to hang, and an answer would not know where to go.

- The architect handles matters one at a time: when the next begins, the last is done and recorded, or is hanging on the
  owner's answer. The wait costs little, since a matter generally takes a few minutes.

- The owner's words, and matters the owner has answered, come first in the queue and are taken up as soon as the one in
  hand is done, with no cell of their own. When the owner speaks while the architect works, it is a higher priority
  question or instruction; when the owner has just decided, the owner is waiting for the result.

- After a new base when one is due and the owner's words and answers (flow/architect/matters.md, "The owner's words, and
  matters the owner has answered, come first"), the architect takes main's red CI, then a matter a stopped work waits
  on, then the rest of its queue, oldest first: a red main holds every merge of the project (backend/tools/ci.md, "The
  engine watches each project's main"), and a stopped work spends its time waiting.

- Each matter must be handled fast enough, since the owner's words wait for the one in hand.

- A report enters the architect's queue as soon as it comes, and an architect not running starts; nothing is saved up,
  and an urgent one reaches it while it runs. Starting when needed costs no more than saving up for an hour.

- While every work is paused, no architect is given a matter for free slots with no item to start, since no new work can
  start.

- Receipts and other small matters are cut at the source, not filtered by another agent. What must shrink is the small
  matters in the architect's hands, and not sending them saves running one more agent.

- What is reported is first made clear, then matched to existing items, and merged where one can take it; only where
  none can and it is urgent is an item created, and otherwise it is considered as an observation. What arrives is not
  always an item and often takes research, and creating by urgency first leaves duplicates once an existing item turns
  out able to take it.

- The architect judges reports itself, with no cheap model sorting them, and so too whether something falls within an
  existing item. Judging that a matter is nothing needs all the architect's context, which a model sorting mechanically
  does not have.

- When report work reaches its end and has written next steps, a matter that is not urgent is queued for the architect,
  with the item, the report's path and merge commit, and the next steps as written. The architect opens a follow-up,
  sets a later look, declines with a reason, or tells the owner. By the end the report is merged and its next steps are
  final; earlier, it could still be sent back.

- A change to items that could lose something (cancelling, merging, splitting, changing an acceptance, touching an item
  being worked on) is checked at once by a second look, a session of its own, and problems go back to that matter to
  fix; other changes take effect after the engine's checks. There is no batch review, no cell for revising, and no
  question at the end of a batch whether to plan again. The engine can check only form, not whether a change is
  reasonable. A second look at every change would cost a fork each, and creating an item wrongly loses nothing.

- An architect's work ends when the queue is empty, with no closing round. Records are left as it goes, so that round
  would be needless and cost money.
