# A work that changes the engine's checks declares them, and the review judges what they find

## What is done now

The engine that judges a work is the one installed, and the flow and its checks of returns are built into it: a change
to one of those checks on a work's branch takes effect only once it is merged and installed. Until then the work is
judged by the check as it was, and its own returns can be sent back by the very fault it mends.

Such a work names, in its implementation's return, each check of the flow's `engine.checks` it changes
(`checks_changed`), with a sentence on why the installed check would send a right return of it back; a fix that changes
one more names that one too. The declaration holds from the return that makes it on: the latest implementation's, with
every fix's since. The engine accepts a declaration only in the project the engine is built from, since no other
project's change can change the engine's checks, and only of checks the flow has, each named once
(`checks_changed_known`). No declaration names `checks_changed_known` or `set_aside_answered`, and what they find is
never set aside: they are what makes a declaration no exemption, the one judging it and the other holding the reviews
to answering each finding.

From then on, a problem in any return of the work that only a declared check found is not sent back: the return is
taken, and the problem is kept on its round (`rounds.set_aside`), numbered in the work (S1, S2, ...). A return that also
fails an undeclared check is sent back with only those problems, and nothing is set aside for it. The installed checks
still run on every return; nothing they find is dropped.

Each finding set aside is judged by a review after it: the details review and the re-check are given those no review
judged yet and answer each, as the old rule's product (`old_rule`, saying why by the diff) or a real problem, which they
also raise as a finding to fix (`set_aside_answered` holds them to answering each once). One found after the details
review, as in adversarial testing, reaches the merge gate unjudged: the gate's value `gates` has the level `set_aside`,
after the items depended on and before the owner's approval, whose row sends those findings to the fix as rows of its
table, which it answers `old_rule` or fixes, and the re-check judges the answers. No work enters the merge queue, nor is
the owner asked to approve it, with a finding set aside that no review judged. The review also checks the declaration
itself against the diff: the engine cannot tell which file holds which check.

A review does not judge what is set aside in its own return, since no review came after it. A finding set aside in a
return of the re-check goes by the merge gate's row `set_aside` to the fix and to the re-check again, and the way round
ends when a return of the re-check sets none aside. The re-check is given the installed prompt, so a return written by
it is reported only where the installed check disagrees with that prompt, which the engineer cannot mend. So the second
time the same check is set aside from a return of the re-check, the fix does not answer that row but asks the advisor,
who judges whether the re-check can write a return that passes the installed check; where it cannot, the architect has
the work wait for the installation or changes the item. The engine does not stop the way round itself: as with the
other loops between the fix and the re-check, the counter `review_returns` tells the owner past its limit and the time
budget pauses the work.

## Why

The constitution's Article 1 has wf-agents develop itself through its own flow, and the policy has a work that mends the
engine's own rules not be stopped by the fault it mends. Before this, such a work was sent back by the installed check,
and went around it by asking the advisor: #217 took seven more rounds from the return sent back until the work was back
at its cell, the advisor's ruling among them. With the declaration it takes two: the fix answers, and the re-check
judges.

The declaration is no exemption. The installed checks still run and what they find is recorded on the round; every
finding is judged by a review against the diff, and the merge gate lets none through unjudged; and only the project the
engine is built from can declare at all. A false declaration has to pass both the details review and the re-check.

## Options not chosen

- Split the change to the check off into an item of its own, merged and installed first, with a dependency that waits
  for the installation: each such change would cost an item, a whole graph and an installation (days, while installing
  is done by hand), and it cannot split a work whose change to the check is the item itself, as #217's was.
- Run the checks in CI with the engine built from the branch: a check reads the live database, the worktree and the
  configuration, none of which leave the machine the engine runs on.
- Install new builds promptly: no installation, however prompt, holds a change that is only on the work's branch, and
  the policy rules out building automatic installation first.

## Challenges turned down

None yet.
