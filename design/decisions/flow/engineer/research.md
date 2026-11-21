# The research

How a work arrives at its plan: the reading of the design tree (R0), the research (R1), the deep research (D1), and the search for a better way (R2, D2).

### decision the-tree-is-read-in-a-cell-of-its-own
R0 exists so that the design tree is read before any work on the item: it understands the item only as far as it needs to know where in the tree to look, and hands in nothing.
- replaced: R0 on a separate line, gathering the history into a brief of at most 500 words for R1 to read in a new session, was on main; the owner changed it, because agents do not read the tree, or not deeply, which is why R0 exists, and what R0 reads stays in the session that R1 goes on in.
- replaced: R0 handing in the decisions related to the item and a supplement to its background was on main; the owner changed it, because their first reason, what a reopened session would lack, was gone once each new session had R0 read the tree again.
- rejected: an agent proposed that R0 also confirm the defect or the need in the code and its history; the owner rejected it, because R0's one purpose is that the tree is read, since agents have skipped it and changed the code at will countless times.
- rejected: an agent proposed putting the whole design tree into the line's session instead of R0's reading; the owner rejected it, because the tree of a larger project would overflow the session at once.

### decision the-research-asks-of-no-other-item-unbidden
R0's and R1's tasks do not send them to look at other items or at earlier work on the item; `query`, which every agent cell may ask for, is there for when the item needs it.
- rejected: an agent proposed that R0 ask `query` for the items around its own and for the item's earlier works, or that the engine give it them as data; the owner rejected it, because most items need only themselves, and the tool is for when it is needed.
- replaced: R1's task opening with an order to ask `query` whether the item was worked on before was on main; the owner changed it, because the first round of every research went on a query, though most items had never been worked on.

### decision the-research-may-make-what-its-problem-needs
R1 and D1 may make code, data and tests for what they research: a test goes with the project's tests, and anything else goes into `project.research`, committed.
- replaced: R1 handing in only a report and changing only `research/`, enforced by a check, was on main; the owner changed it, because the limit is there to keep the research on its problem, not to guard against the agent.
- rejected: an agent proposed bringing back the old research's rules for `research/`, such as deleting empty probes; the owner rejected it, because the engineering rules already say whom a file serves and when it is removed.

### decision deep-research-only-for-a-fact-or-a-trial
R1 sets `deep_needed` to yes only when the direction rests on a fact it cannot check in a round or two, or on trying several ways first; a choice for the owner is a question.
- replaced: Every work going through research and then a design stage was on main; the owner changed it, because the research concludes simple things itself, and only very complex ones need the deep research.
- replaced: Entering the deep research on any approval, open choice or contradiction was on main; the owner changed it, because the deep research is used only where it is needed: of ten works that entered it in one night, six should not have.

### decision the-way-forks-before-the-research
R1's `deep_needed` says whether the deep research is needed, and is a condition of R1 and D1 alike: R1 runs while it is no, D1 once it is yes, and whichever runs answers the plan's dialogues.
- replaced: Running R1 and D1 one after the other, with D1 entered after R1, was on main; the owner changed it, because the fork that R1 decides belongs before R1, so that a later pass goes from R0 to R1 or to D1, without two sets of blocks.
- rejected: an agent proposed that the plan's judges write their points into one block for the research and another for the deep research, depending on which held the plan; the owner rejected it, because nothing stops them from writing into the wrong one.
- rejected: an agent proposed leaving the design's paragraph on a maker's dialogues as it was, naming no maker where two cells may write the plan; the owner rejected it, because that paragraph was not true where two cells may write the plan.

### decision the-research-hands-over-with-what-it-found-alone
When `deep_needed` is yes, R1's return holds only `found`, with no plan, question or answer, as `deep_needed` declares (`with_nothing_but`); the engine sends back one that holds more, saying what.
- replaced: Telling R1 to write only `found` when it says yes, which the engine did not check, was on main; the owner changed it, because a question R1 handed in while deciding on the deep research left no cell for the owner to speak to about it, and the deep research excludes every other output.
- rejected: an agent proposed that a research sending the work to the deep research still hand in its plan and questions; the owner rejected it, because a research that cannot decide on a design writes no draft of one, and its questions are set in the draft.
- rejected: an agent proposed that one return carry the answer to the owner along with `found`; the owner rejected it, because the rule has the cell answer the owner first, and decide on the deep research in a later return.

### decision the-better-way-is-looked-for-once
R2 and D2 look for a better way once in each work, until one of their runs has handed in (`{cell}_handed_in`), and give it to the plan's writer in `better_found`, in a few sentences, or give nothing.
- replaced: R2 and D2 running on each change of the plan, writing the plan themselves, asking questions and being spoken to, was on main; the owner changed it, because later changes to a plan are small, and in one work R2 ran four times and handed in nothing.
- rejected: an agent proposed that R2 watch `deep_needed`, whose first version runs a cell once; the owner rejected it, because the cell would then be run by what it does not read.
- rejected: an agent proposed asking whether there is a better way in the plan's review, on another line; the owner rejected it, because when the question is asked again in the writer's own session, a better way is often found: in the old flow, 40 times in 52.
- rejected: the owner proposed that `better_found` only mark that a better way was found, since its writer knows the rest from the session; the owner rejected it, because the way is written out in case the session was replaced, and for the owner to see, though no longer than needed.
- rejected: an agent proposed accepting that no one looks for a better way again once a plan changes greatly; the owner rejected it, because a plan changed that much is a new item, and the old one is cancelled.

### decision the-better-way-is-asked-by-two-cells-with-one-task
R2 and D2 are two cells, one on each line that writes the plan, and they share one task, written once in `tasks/better_way.md`.
- rejected: an agent proposed one cell running on the line of whichever cell wrote the plan; the owner rejected it, because a cell is on one line, and the question must be asked in the session of the plan's writer.
