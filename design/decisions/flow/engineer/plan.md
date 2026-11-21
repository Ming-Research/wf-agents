# The plan

What the plan holds, its conditions of acceptance and its changes of the design tree, and how it is reviewed before it is built (P1, P2).

### decision the-owners-plan-has-no-set-length-or-sections
`plan_owner` is written in the owner's language with no limit of length and no set sections, as the plan now stands, naming none of the code's files, types and functions.
- rejected: an agent proposed a plan for the owner of at most 500 characters in set sections; the owner rejected it, because the plan is all he learns the item from, and a short one he cannot follow.
- rejected: an agent proposed sections for it to follow: what is done, the trade-off, what to watch, and where it differs from the item; the owner rejected it, because another work may not be explained best by these sections.
- rejected: an agent proposed rewording what `plan_owner` holds, or saying what the owner uses it for, to keep the code's names and the plan's history out of it; the owner rejected it, because a fork of the research bore out neither.

### decision another-way-is-told-only-where-it-was-real
`plan_owner` says what another way was, and where the plan wins over it, only where that way was a real choice.
- rejected: an agent proposed that the plan always say what it was compared with; the owner rejected it, because ways are not made up to be compared: only a way that truly is there is told.

### decision acceptance-is-for-the-reviews
The acceptance conditions are numbered sentences, each plainly true or false and saying how it is verified, written for the reviews and the testing and not shown to the owner.
- rejected: an agent proposed leaving the conditions out of what the research hands in, for the implementation to set; the owner rejected it, because the implementation setting its own conditions sets itself its exam, and they are for the reviews.
- rejected: an agent proposed that the adversarial testing write the conditions as tests before the implementation; the owner rejected it, because complete tests cannot always be written before implementing, while what to test can, in plain words the final tests must match.
- replaced: A condition verified by a fast-CI job was on main; the owner changed it, because the fast CI only makes CI faster, and which CI runs a test is the engine's choice, not the plan's.

### decision acceptance-may-change
The conditions a work is accepted by are not locked: they may change while the work goes on, and each review is given what changed in them since the version it last saw.
- replaced: The conditions locked once the first design set them, against an agent quietly loosening them, was on main; the owner changed it, because locking was too strict, and a review found too loose can be tightened later.

### decision every-tree-change-of-a-plan-is-one-question
The plan's writer asks the owner of every change of the design tree in `tree_edits`, of any kind, in one question: each change's file and entry, its diff in a ```diff block, and a sentence.
- replaced: Asking only of a change of a principle or of the constitution, the decisions being the work's to change, was on main; the owner changed it, because protected files and the design tree are two things: a principle is both, and a decision is in the tree and not protected.
- rejected: an agent proposed that the plan only name a change of a principle in `plan_owner`, the engine alone asking of it once the change was made; the owner rejected it, because the plan's writer must ask of every change of the tree, the engine asking apart of the protected files changed.
- rejected: an agent proposed telling the cells that write `tree_edits` to ask in each one's task; the owner rejected it, because the block's own words reach every cell that writes it from one place.
- rejected: an agent proposed a question for each change; the owner rejected it, because several changes retold in crowded questions read as if nothing were said clearly, and a diff shown inline needs only its difference explained.
- rejected: an agent proposed longer words, with a labelled sentence on what the diff changes and why, and options to change so or not; the owner rejected it, because they were too long, and the labels said nothing of what changed.

### decision the-tree-review-looks-for-an-unasked-tree-change
P2 points out any change of the tree that `tree_edits` holds, of the constitution, a principle, a decision or a document, that no question asks the owner about.
- replaced: P2 looking only for a change of a principle or of the constitution that no question asked of was on main; the owner changed it, because the review must catch what the writer fails to ask: in one work four files of decisions and a document changed unasked.

### decision both-plan-reviews-review-every-plan
P1 and P2 review every plan before it is built, on each version of it, whether the research or the deep research wrote it.
- replaced: P1 only after the deep research, and P2 only where the plan changed the tree, was on main; the owner changed it, because then nobody reviewed a plan with neither against the tree's decisions before its code was reviewed.
- rejected: the owner proposed that P2 be run by `tree_edits` alone; the owner rejected it, because a plan that changes no entry of the tree may still need its review, such as one that goes against a decision.

### decision the-plan-reviews-are-split-by-their-yardstick
P1 judges the plan by what `background` asks and the engineering rules, and P2 against the design tree, which is P2's alone; each answers its numbered points in its verdict's why.
- rejected: an agent proposed one cell reviewing a plan against `background`, the engineering rules and the tree together; the owner rejected it, because a cell that does more does worse, the more so where it need not answer each point.
- replaced: P1's point on a decision the plan goes against with no entry was on main; the owner changed it, because one thing is judged in one place, the plan against the tree being P2's.
- rejected: an agent proposed that both reviews judge the plan against the tree after the deep research, accepting that they overlap; the owner rejected it, because the work of the two reviews was to be split plainly.
