# The change

How the plan is built (I1), and how the change is reviewed and tested before the merge (T1 to T4).

### decision the-implementation-waits-for-the-plans-verdicts
I1 runs only on a plan its two reviews passed: it requires `plan_ok` and `tree_ok` to be yes.
- replaced: Holding I1 back by the order of the cells alone was on main; the owner changed it, because after the work goes back, a pass runs every cell, and without the gate I1 would build a plan that its review had just failed.

### decision the-implementation-doubts-the-plan-to-its-writer
Where the plan does not hold, I1 does not work around it: it says where and why in `plan_doubts`, its dialogue with the plan's writer, and writes there again only where a doubt still stands.
- replaced: A list in which the implementation declared where it departed from the design was on main; the owner changed it, because an implementation that knows it departs from the design should fix that, not leave it for the review to challenge.

### decision the-implementation-writes-the-tree-from-the-owners-choices
I1 writes into the design tree the option taken in each answered question marked `tree`, as the tree's rules say; an entry may follow main's tree in wording and place, but never in meaning.
- replaced: I1 writing into `design/` what `tree_edits` held was on main; the owner changed it, because a change to the tree is only the result of the owner's answer.
- rejected: an agent proposed that the engine write the approved entries mechanically, sending the work back when main's text no longer matched; the owner rejected it, because a mechanical rule is too strict, and the review of the change catches any drift.

### decision a-dispute-over-a-fact-is-settled-by-a-run
I1 answers each point of a review or of the testing, fixed with its commit or why not, and a run settles a dispute over a fact; where the two still differ, the owner writes the verdict.
- rejected: an agent proposed sending a disagreement with a review straight up to a senior round; the owner rejected it, because the two sides settle it between them first, as with the adversarial tests, and only what they cannot agree on goes up.
- rejected: an agent proposed a loop in which each side speaks twice before a stronger agent rules; the owner rejected it, because no data showed that one turn each was too little, and the loop adds two rounds to each disagreement.
- rejected: the owner proposed deleting `plan_ok` and `testing_ok`, which no agent reads; the owner rejected it, because the engine reads them at the gates before the merge, and the owner writes a verdict in them to settle a dispute.

### decision the-change-is-reviewed-in-three
In every work, three cells on one line review the change, T1 the code, T2 the tests and evidence, T3 against the design tree; each numbers its questions and answers each by its number.
- replaced: One code review asking its questions in one paragraph was on main; the owner changed it, because mixed together they are done badly: forks of one such review left five of its questions unanswered, with no trace.
- rejected: an agent proposed giving the one review's questions as a numbered list, or as twelve questions in four kinds with the least important merged; the owner rejected it, because mixed in one review they are done badly, however they are listed.
- replaced: A review that wrote all its points at once, and nothing where nothing was wrong, was on main; the owner changed it, because a question went unanswered with no trace, one in a long sentence passed with one yes, and one never checked passed on the engineer's word.
- rejected: the owner proposed no review after the implementation, to get a first running version going; the owner rejected it, because the review after the implementation is the design tree's most critical step, and the flow serves real work, so it cannot be half made.

### decision each-review-of-the-change-is-told-what-is-not-its-own
Each of T1, T2 and T3 is told in one short line what is not its part, such as T1's "Do not review the tests or the design tree.", and nothing about the other reviews.
- rejected: an agent proposed for each review a sentence naming the others' part and telling it to leave out even a point it could make about either; the owner rejected it, because it was too long.
- rejected: the owner proposed saying, for each part left out, that the other two reviews have it; the owner rejected it, because that tells the agent the flow has other cells, against principle an-agent-knows-only-its-task.

### decision the-code-review-names-no-principle
T1's second question asks whether the code is well built by the standards of software engineering, naming no principle; its answer by number says what it checked and found.
- rejected: an agent proposed naming the practices to judge by, such as nothing written twice and each responsibility in one place; the owner rejected it, because a reviewer then looks only for those named, and there are more than any list holds.
- rejected: the owner proposed a separate question on best practice; the owner rejected it, because a seventh question would cover the same ground as the second.
- replaced: T1's question whether the change was sound in structure and in its lines was on main; the owner changed it, because naming principles narrows what a reviewer looks for, and the agent knows what the standards ask.

### decision the-code-review-judges-protected-changes
T1 asks whether each change to a file the project protects is one the work needs, and the read-only lines are told which paths the project protects.
- replaced: No review of changes to protected files, with the plan's tree review looking only for a principle changed without asking, was on main; the owner changed it, because the reviews must judge such changes too.

### decision the-design-tree-review-of-the-change-asks-four-things
T3 asks what in the tree the change goes against unasked, whether each answer marked `tree` is written and each other tree change mends an untrue entry, what it chose unasked, and what is left untrue.
- replaced: Checking the entries against `tree_edits`, and looking for an approach turned down, something done on purpose or a relation between two places left unrecorded, was on main; the owner changed it, because every choice of his goes into the tree, and nothing else does.
- rejected: an agent proposed that T3 also ask whether an entry is longer than it needs to be; the owner rejected it, because a work goes back from T3 no further than I1, while R1 and D1 write `tree_edits`, so the writer must know what to leave out.

### decision the-reviews-of-the-change-and-the-testing-keep-their-own-lines
T1, T2 and T3 run on the `code_reviewer` line, which only reads, and T4 on the `tester` line, which changes only tests; neither is the plan review's line.
- rejected: the owner proposed running the change's reviews on the plan review's line, to save reading again what that line had read; the owner rejected it, because that line runs one level up and only reads, so the code review would cost more there and the testing could not commit its tests.
- rejected: the owner proposed joining the code review's and the testing's lines; the owner rejected it, because the review would gain the right to write tests and the testing would lose its fresh eyes.

### decision the-reviews-take-the-head-and-the-head-runs-the-check
The reviews of the change take `head`, the work's head, as part of the change they watch, and read the diff from main to it; the head, not what a block names, runs the check before the merge.
- replaced: Agents naming commits in blocks, with the check matching the head against any block that named it, was on main; the owner changed it, because what matters is that what merges was reviewed: the review reviews the branch's head, and the head reviewed is the one merged.
- rejected: an agent proposed counting only the blocks marked as naming the head, `code` and `tests`; the owner rejected it, because reviewing the branch's head is enough, and the marks made the design complex and full of holes.

### decision testing-is-asked-for-a-change-that-goes-wrong-silently
T2 says in `needs_testing` whether the change is of a kind that goes wrong silently, which only tests written to break it find, and names in its why the kind the change touches.
- replaced: A separate cell deciding whether a work is tested adversarially was on main; the owner changed it, because it said yes 49 times in 60, and the review answers the question with one yes or no: whether the change is of a kind that fails silently.
- rejected: an agent proposed a project switch that turns the testing off, or a list of paths whose change calls for it; the owner rejected it, because a setting is static, and someone should answer the question for each change.
- rejected: the owner proposed that the architect write the standard for answering the question, with the project's policy; the owner rejected it, because a project's threshold barely changes, and a text tuned once is spoiled by the next edit.

### decision the-adversarial-testing-runs-when-asked
T4 runs when T2 says the change needs testing, when the acceptance changes, and when I1 answers its points: not on each version of the code.
- replaced: Running the adversarial testing on each version of the code was on main; the owner changed it, because run that way, half of its 178 runs found nothing, while run on request it ran once, and once more after the fix.

### decision the-testing-mends-only-its-own-tests
T4 writes only tests, each naming its independent reference; it checks its own test that is red after the change against that reference first, and reports a red test of the engineer's in `testing`.
- rejected: an agent proposed that the testing fix and rerun any test that fails, the engineer's too; the owner rejected it, because the testing fixes only its own tests and writes the engineer's into `testing`, as the old system did.
