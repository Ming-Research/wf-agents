# The check and the merge

What the check before the merge (M1) runs on and how what it finds is answered, and when the work stops before it.

### decision the-check-is-answered-by-a-commit-or-a-question
I1 answers a red check by fixing it and committing, or, where it judges the red not this change's to fix, by asking the owner; no block holds its answer.
- replaced: A block in which I1 answered each point of the check, fixed or why not, which ran the check again, was on main; the owner changed it, because no cell read it: the check reads no words, a fix moves the head, and a why-not only looped on one commit.
- rejected: an agent proposed a rule that a cell woken by words asking for a reply must write that reply; the owner rejected it, because the reply to the check is read by no cell, and is deleted instead.

### decision the-check-runs-again-only-on-what-it-checks
M1 runs again only on a new head, a new main or a changed answer to a question, never on what a cell says in words.
- replaced: The check run again on any answer of I1's, a commit or not, was on main; the owner changed it, because running a failed check again with nothing changed destroys a flaky failure's scene and lets it merge, when what is needed is to find it.
- rejected: an agent proposed a block in which the owner could have a failed check run again where I1 changed nothing; the owner rejected it, because the problem is finding the flaky failure, not running again, and words typed to run a check mean nothing.

### decision a-work-with-no-change-waits-before-the-check
A work whose implementation hands in no change waits before the check, for the owner's verdict or more implementation: no review is made to run on an empty change.
- rejected: an agent proposed that the code review run at least once in each work, for example by watching the plan; the owner rejected it, because the work then stops plainly before the check, rather than a review passing an empty change.
