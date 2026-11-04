# What the owner sets to be done before a merge is recorded by the owner's tick and by the architect

## What is done now

Something the owner sets to be done before an item's work merges becomes a row of `merge_conditions` in two ways, side by
side. The owner ticks "Must be met before it merges" on the box the words are written in: the box at the bottom of a
work's group, the small box that answers an engineer's ask or question, and the box that replies to a question of the
architect's about one item. A line then holds the sentence of the condition, the box's words until the owner writes there,
since the words may be only "OK"; an empty line keeps them from being sent. And the architect records one with the item
change `add_merge_condition`, from any record it is given: an answer to its ask, the owner's words to it in the one-to-one
or in a group, words a delegate relayed, words said before the work had a group. Whoever records it writes the sentence;
each row keeps the record its words are in, those words as the record has them, and who the record says said them (the
owner, the owner relayed by the delegate, the delegate in its own words, or unrecorded), so that nothing is written as the
owner's that the record does not say. The architect's change is refused when the words are not found letter for letter in
the record, or the record is the architect's own words.

The engineer records nothing: when it hears the owner wanting something done before the merge and the Merge conditions it
is given do not hold it, it asks the architect to record it, as every cell of its lines can but `A7`, which only says how
long a question goes on waiting and goes back to the cell that asked it, where it can.

**The merge gate holds what is recorded, and nothing else.** Words that set a condition, neither ticked by the owner nor
recorded by the architect, hold nothing at the gate. So the architect's rules and the description of its change say: when
words of the owner's you are given make something a thing to be done before a merge, record it in this round; and the
engineer's rules say to ask the architect as above. A condition recorded after the work has passed the gate, while the merge
step waits its turn or for its full CI, holds the merge all the same: that step reads the conditions last of all, just before
the merge call, and goes back to the gate (the merge step's row `condition_set`). Only one recorded during the merge call
itself comes too late, the window of a tick and of the architect alike, and this is accepted. Once the item's merge stands,
both are refused, since nothing is left to hold.

## Why

Before this, such a condition stood only in words: a line of an item's decisions, the owner's words in a group, the answer
to an ask. The merge gate read none of them, and #217 merged with its condition not met, the architect's pause coming one
cell too late. Delivering what the owner did not let through is worse than stopping (the policy's first principle).

Recording and judging are kept apart because their mistakes cost differently. A condition recorded by mistake, or too
strictly, only holds the work once more, and the owner withdraws it; so the way in is wide: whoever records it, a delegate's
relayed words included, and the sentence written by whoever records it. A condition judged by mistake merges wrongly; so the
way of judging is narrow: a chosen option at the merge gate's ask, kept with who chose it
(`decisions/merge-conditions-judged-by-an-option-at-the-gate.md`).

Article 7: each step from words to a held merge is a row or a record the owner can follow. Article 2: the sentence and the
record of the words are there wherever the condition is read. Article 8: the tick is one part in each box where the owner
already writes, not a form of its own.

## Options not chosen

- Have agents pick conditions out of the words: each engineer's return and the architect's change carry a field of the
  conditions heard, the engine checking each quotation, and the owner's page stays as it is. It rests on an agent noticing,
  in the middle of other work, that a sentence is a condition, and a missed one is silent, as today; the same words could be
  recorded twice; and every engineer's cell would carry one more field.

## Challenges considered and turned down

None yet.

## How it was decided

The owner's assistant kept this option at the review of the design of item #261, on the owner's behalf. In translation, from
the review's answer in the work's round R309-19: "P1 and P2 stay as chosen last time" (the earlier words: "P1 is the owner's
tick and the architect's record, the two ways side by side"), with the owner's requirement that the gate's limit above be
written down. A different choice by the owner replaces this file.
