# Only the owner's own answer judges a merge condition

## What is done now

An answer to the merge gate's ask of a merge condition judges it only when the answer is declared as the owner's own
(`speaker` owner) and chose an option, met or withdrawn. The option is a field of the answer (`choice`), never read out of
its words. An answer of a delegate, a session that speaks for the owner, judges nothing, whichever option it chose: it goes
to the engineer as words, saying the delegate gave it; the condition stays open and the work goes on waiting at the gate. This
is said in one place, the view `item_merge_conditions`, so that changing the rule changes that one definition, and answers
already kept are read by the new rule.

This rule is for this level of the gate alone. The owner's approval of a change at the same gate (the level `owner`), and the
owner's review of a design at cell `D3`, take a delegate's answer, and are not changed by this. So one merge gate holds two
rules on whose answer counts.

## This was chosen by following the recommendation: the owner has not answered it

This choice is about what the owner lets the delegate do, which the delegate cannot settle for itself, and the owner has not
answered it. The delegate's words at the review of the design of item #261: "P3 主人本人答之前实现按「只认主人本人」，合并不等；主人后来选代答也算就另开小条目；写进 decisions/ 时照实写 P3 按推荐走、主人本人还没答。" and
"P3 是助理自己的权限，留给主人本人定。" The work does not wait for the answer. If the owner later chooses that a delegate's answer counts
too, that is a small item of its own, which rewrites this file.

## Why

A condition is what the owner kept for themselves to judge, and a wrong judgment merges what the owner did not let through,
which may have to be reverted and which no one may notice (the policy's first principle). Waiting costs a night: while the
owner is away, a work with a condition waits at the gate holding no slot, and only two such works have been seen, #217 and
#236. A delegate that answers costs one round of the choice of the way each time. Recording stays open to the delegate, as
`decisions/merge-conditions-recorded-by-tick-and-architect.md` says: a condition recorded by mistake only holds the work once
more.

## Options not chosen

- A delegate's answer counts too, kept and shown as given for the owner. No work waits while the owner is away, which fits the
  owner's words that nothing should be held up at night (ask 47, as relayed). But then a merge can only be taken back by
  reverting it, and "only the owner judges" becomes "the owner or the delegate judges".

## Challenges considered and turned down

None yet.
