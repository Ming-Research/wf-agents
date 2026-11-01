Decision way-cells: Where a work goes by what the project's configuration says (whether it keeps records, whether the branch changed them, whether it names something under test, whether it names other projects), a cell of the engine's own chooses, with a row each way and the setting among its inputs (`D2`, `D5`, `D8`, `D12` and `A8` of the engineer's graph, and the table review's rows to `T5` or `M1`), instead of a gate that states what it needs and that the engine answers by itself when the project lacks it, because a work that went wrong is traced by its rows, and a switch inside another cell is behaviour the chart does not show; an engine's cell runs no agent, so the round such a gate saved is saved still.

Rests on: C:flow-does, C:efficient

Rejected:
- A gate that states what it needs from the project's configuration, which the engine answers down its not-applicable row or its outcome that goes on when the project lacks it, recording that it did: rejected because the chart drew the way into the gate and the engine took the way out of it unseen, a hidden switch, where a cell of the engine's own shows the same choice as rows.

Decision gates-judge-change: On a redesign, each check after design (`D9`, `D10`, `D11`, `D13`, `D14`) is given what the design changed since it last answered (`design_change`): the engine compares, field by field, the design the check was given at its last round, the latest written before it, with the design now, sentence by sentence within each field's text, and gives the sentences removed and added, with the check's answer then and why, beside the whole design; the check then judges whether the change alters its last answer, handing that answer in again, why saying the change does not touch it, or judging the change; a difference no shorter than the design is not given, and the check judges the whole design as at the first design; and the design written again keeps word for word the text of what does not change; instead of every check judging the whole design again at every redesign, because what a redesign did not change was judged already, and a check given the whole design again judged it again, while the whole design stays beside the change since a local change can move an answer about the whole, such as its cost or its size.

Rests on: C:efficient, C:information, F:redesign-gates-whole, F:design-rewritten-whole

Rejected:
- Every check after design judging the whole design again at every redesign: rejected because it spends a round of every check on what the redesign did not change.
- The difference of the design's JSON, or of each field's text by line: rejected because a field is one line of the JSON and a paragraph one line of its text, so that one sentence changed shows the whole of them again.

Decision gate-na-no-column: A check after design that the work's way did not reach, as the gap in what is under test for a project with nothing under test, is no column of the table review, instead of a column asking whether its reason holds, because nobody judged it: the project's configuration chose the way past it.

Rests on: C:problem-first, D:way-cells

Decision work-own-change: Where the engine judges what a work changed from one commit to another (the checks that a round changed nothing outside `research/`, nothing in the design paths or nothing outside the test paths, the check that a round hands in a new design card when the design paths changed since the last one, whether the branch changed the project's records, and whether the design tree changed after the owner confirmed the design card), it counts only the work's own change: a file changed between the two commits that the later one also changes against main as merged into it, the merge base of the later commit and the fetched main; instead of every file the two commits differ in, because a merge of main, to catch up with it or to resolve a conflict, brings in main's changes, and counted as the work's they sent a return back for a design card or a change the work never made. A file both main and the work changed stays the work's, because main's change to it can change what the work's change there means. What is counted against main from the start of the branch is already counted from the merge base and stays so.

Rests on: C:information, F:main-merge-sent-back
