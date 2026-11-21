# The change

How the plan is built (I1), and how the change is reviewed and tested before the merge (T1 to T4).

### decision the-implementation-waits-for-the-plans-verdicts
I1 runs only on a plan its two reviews passed: it requires `plan_ok` and `tree_ok` to be yes.
- replaced: The order of the cells alone holding I1 back stood on main; the owner changed it, because a pass runs every cell after a way back, and without the gate I1 would build a plan its review had just failed.

### decision the-implementation-doubts-the-plan-to-its-writer
Where the plan does not hold, I1 does not work around it: it says where and why in `plan_doubts`, its dialogue with the plan's writer, and writes it again only where a doubt still stands.
- replaced: A list in which the implementation declared where it departed from the design stood on main; the owner changed it, because an implementation that knows it departs should mend it, not keep it for the review to challenge.

### decision the-implementation-writes-the-tree-as-the-plan-says
I1 writes into `design/` what `tree_edits` holds: an entry may follow main's tree in wording and place, never in meaning.
- rejected: an agent proposed that the engine write the approved entries mechanically, sending the work back where main's text no longer matched; the owner rejected it, because a mechanical rule binds too tightly, and the review of the change catches a drift.

### decision a-dispute-over-a-fact-is-settled-by-a-run
I1 answers each point of a review or of the testing, fixed with its commit or why not, a dispute over a fact settled by a run; where the two still differ, the owner writes the verdict.
- rejected: an agent proposed sending a disagreement with a review straight up to a senior round; the owner rejected it, because the two sides settle it between them first, as with the adversarial tests, and only what they cannot agree on goes up.
- rejected: an agent proposed a loop in which each side speaks twice before a stronger agent rules; the owner rejected it, because no data showed once each to be too little, and the loop adds two rounds to each disagreement.
- rejected: the owner proposed deleting `plan_ok` and `testing_ok`, which no agent reads; the owner rejected it, because the engine reads them at the gates before the merge, and the owner writes a verdict in them to settle a dispute.

### decision the-change-is-reviewed-in-three
On every work the change is reviewed by three cells on one line, T1 the code, T2 the tests and evidence, T3 against the design tree, each numbering its questions and answering each by number.
- replaced: One code review asking its questions in one paragraph stood on main; the owner changed it, because mixed together they are done badly: forks of one such review left five of its questions unanswered with no trace.
- rejected: an agent proposed the one review's questions as a numbered list, or as twelve in four kinds with the least weighty merged; the owner rejected it, because mixed in one review they are done badly however they are listed.
- replaced: A review writing every point at once and nothing where nothing was wrong stood on main; the owner changed it, because a question went unanswered with no trace, one in a long sentence passed with one yes, and one not checked passed on the engineer's word.
- rejected: the owner proposed no review after the implementation, to get a first runner going; the owner rejected it, because the review after implementation is the design tree's most critical step, and the flow serves real work and cannot be half made.

### decision each-review-of-the-change-is-told-what-is-not-its-own
Each of T1, T2 and T3 is told in one short line what is not its own, as T1's "Not the tests and not the design tree.", and nothing of the other reviews.
- rejected: an agent proposed a sentence for each naming the others' part and to leave out even a point it could make of either; the owner rejected it, because it was too long.
- rejected: the owner proposed saying of each part left out that the other two reviews have it; the owner rejected it, because that tells the agent the flow has other cells, against principle an-agent-knows-only-its-task.

### decision the-code-review-names-no-principle
T1's second question asks whether the code is well built by the standards of software engineering, naming no principle; its answer by number says what it checked and found.
- rejected: an agent proposed naming the practices to judge by, such as nothing written twice and each responsibility in one place; the owner rejected it, because a reviewer then looks only for those named, and there are more than any list holds.
- rejected: the owner proposed a question of its own on best practice; the owner rejected it, because a seventh question would cover the second's ground twice.
- replaced: T1's question whether the change was sound in structure and in its lines stood on main; the owner changed it, because principles named narrow what a reviewer looks for, and the agent knows what the standards ask.

### decision the-code-review-judges-protected-changes
T1 asks whether each change of a file the project protects is one the work needs, and the lines that only read are told which paths the project protects.
- replaced: No review of the changes of protected files, the plan's tree review looking only for a principle changed unasked, stood on main; the owner changed it, because the reviews must judge such changes too.

### decision the-design-tree-review-of-the-change-asks-four-things
T3 asks what the change breaks in the tree, whether its entries say what `tree_edits` says and no more, what it leaves untrue, and what choice it leaves unrecorded.
- rejected: an agent proposed that T3 also ask whether an entry is written longer than it needs; the owner rejected it, because a work goes back from T3 to I1 at most, while `tree_edits` is written by R1 and D1, so the one who writes must know what to leave out.

### decision the-reviews-of-the-change-and-the-testing-keep-their-own-lines
T1, T2 and T3 run on the `code_reviewer` line, which only reads, and T4 on the `tester` line, which changes only tests; neither is the plan review's line.
- rejected: the owner proposed running the change's reviews on the plan review's line, to spare reading again what it read; the owner rejected it, because that line runs a level up and only reads, so the code review costs more there and the testing could not commit its tests.
- rejected: the owner proposed joining the code review's and the testing's lines; the owner rejected it, because the review would gain the right to write tests and the testing lose its fresh eyes.

### decision the-reviews-take-the-head-and-the-head-runs-the-check
The reviews of the change take `head`, the work's head, in the change they watch and read the diff from main to it; the head, not what a block names, runs the check before the merge.
- replaced: Agents naming commits in blocks, the check matching the head against any block naming it, stood on main; the owner changed it, because what is wanted is that what merges was reviewed: the review reviews the branch's head, and the head reviewed is the one merged.
- rejected: an agent proposed counting only the blocks marked as naming the head, `code` and `tests`; the owner rejected it, because reviewing the branch's head is enough, and the marks made the design complex and full of holes.

### decision testing-is-asked-for-a-change-that-goes-wrong-silently
T2 says in `needs_testing` whether the change is of a kind that goes wrong silently, which only tests written to break it find, and names in its why the kind the change touches.
- replaced: A cell of its own deciding whether a work is tested adversarially stood on main; the owner changed it, because it said yes 49 times in 60, and the review answers it as one yes or no: whether the change is of a kind that fails silently.
- rejected: an agent proposed a project switch turning the testing off, or a list of paths whose change calls for it; the owner rejected it, because a setting is static, and someone should answer the question for each change.
- rejected: the owner proposed that the architect write the standard the question is answered by, with the project's policy; the owner rejected it, because a project's threshold barely changes, and a text tuned once is spoiled by the next edit.

### decision the-adversarial-testing-runs-when-asked
T4 runs when T2 says the change needs testing, when the acceptance changes, and when I1 answers its points: not on each version of the code.
- replaced: The adversarial testing run on each version of the code stood on main; the owner changed it, because run so, half of its 178 runs found nothing, while run on request it ran once and once more after the fix.

### decision the-testing-mends-only-its-own-tests
T4 writes only tests, each naming its independent reference: a test of its own red after the change it checks against that reference first, and of the engineer's it says in `testing`.
- rejected: an agent proposed that the testing fix and run again any test that does not hold, the engineer's too; the owner rejected it, because the testing mends only its own tests and writes the engineer's into `testing`, as the old system had it.
