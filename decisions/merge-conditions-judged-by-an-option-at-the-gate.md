# An option chosen at the merge gate's ask judges a merge condition, the owner's or the delegate's

## What is done now

An answer to the merge gate's ask of a merge condition judges it when it chose an option, met or withdrawn, and is declared as
the owner's or as a delegate's, a session that speaks for the owner. The option is a field of the answer (`choice`), never read
out of its words. An answer a delegate gave is marked as the delegate's where the judgment is kept and shown: the view
`item_merge_conditions` says who judged it (`judged_by`), and the item's page says "by the delegate". Words with no option judge
nothing, whoever says them: they go to the engineer, saying who said them, and the gate asks again. The rule is said in one
place, the view, so that changing it changes that one definition, and answers already kept are read by the new rule.

So the merge gate has one rule on whose answer counts: the owner's approval of a change at the same gate, and the owner's review
of a design, take a delegate's answer too.

## Why

The owner's own answer at the work's group, on the design of item #261, in translation from the interjection to round R309-58:
"You are the assistant: what is marked as relayed by the assistant counts. It does not need me myself." The owner is often away,
and a work with a condition would otherwise stand at the gate until the owner came back. The mark keeps what a wrong merge
needs to be traced: who judged it, and on which ask.

## Options not chosen

- Only the owner's own answer judges; a delegate's goes to the engineer as words and the condition stays open. A wrong judgment
  is hardest this way, and it was the recommendation at the review. But every work with a condition waits for the owner while the
  owner is away, each answer of the delegate's costs a round of the choice of the way, and the same gate would hold two rules on
  whose answer counts. The implementation first went this way, by the recommendation, until the owner chose.

## Challenges considered and turned down

None yet.
