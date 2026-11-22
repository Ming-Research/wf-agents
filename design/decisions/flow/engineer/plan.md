# The plan

What the plan holds, its acceptance conditions and its changes to the design tree, and how P1 and P2 review it before it is built.

### decision the-owners-plan-has-no-set-length-or-sections
`plan_owner` describes the plan as it now stands, in the owner's language, with no length limit and no set sections, and names none of the code's files, types or functions.
- rejected: an agent proposed a plan for the owner of at most 500 characters, in set sections; the owner rejected it, because the plan is all he learns the item from, and he cannot follow a short one.
- rejected: an agent proposed sections for it to follow: what is done, the trade-off, what to watch, and where it differs from the item; the owner rejected it, because another work may not be explained best by these sections.
- rejected: an agent proposed rewording what `plan_owner` holds, or saying what the owner uses it for, to keep the code's names and the plan's history out of it; the owner rejected it, because a fork of the research supported neither.

### decision another-way-is-told-only-where-it-was-real
`plan_owner` says what another way was, and where the plan is better than it, only when that way was a real choice.
- rejected: an agent proposed that the plan always say what it was compared with; the owner rejected it, because ways are not made up to be compared: only a way that really exists is described.

### decision acceptance-is-for-the-reviews
The acceptance conditions are numbered sentences, each plainly true or false and saying how it is verified, written for the reviews and the testing and not shown to the owner.
- rejected: an agent proposed leaving the conditions out of what the research hands in, for the implementation to set; the owner rejected it, because an implementation that sets its own conditions writes its own exam, and the conditions are for the reviews.
- rejected: an agent proposed that the adversarial testing write the conditions as tests before the implementation; the owner rejected it, because complete tests cannot always be written before the implementation, but what to test can be, in plain words that the final tests must match.
- replaced: A condition verified by a fast-CI job was on main; the owner changed it, because the fast CI only makes CI faster, and which CI runs a test is the engine's choice, not the plan's.

### decision acceptance-may-change
The conditions a work is accepted by are not locked: they may change while the work goes on, and each review is given what changed in them since the version it last saw.
- replaced: Locking the conditions once the first design set them, so that an agent could not quietly loosen them, was on main; the owner changed it, because locking was too strict, and a review found too loose can be tightened later.

### decision each-choice-of-the-owners-is-asked-in-a-question-of-its-own
The plan's writer asks the owner about each choice the tree's rules give him, each in a separate question about what to do; the change to the tree is not part of the question.
- replaced: Asking about every change to the tree in `tree_edits`, all in one question with each change's diff, was on main; the owner changed it, because a question should be about what to do, and the tree change is only its result.
- replaced: Asking only about a change to a principle or to the constitution, since the decisions were the work's to change, was on main; the owner changed it, because protected files and the design tree are two things: a principle is both, while a decision is in the tree but not protected.
- rejected: an agent proposed that the plan only name a change to a principle in `plan_owner`, with the engine alone asking about it once it was made; the owner rejected it, because the plan's writer must ask about every tree change, and the engine asks separately about changed protected files.

### decision the-tree-review-judges-only-what-the-plan-goes-against
P2 judges only what the plan goes against in the constitution, the principles and the decisions; going against an entry breaks nothing when a question puts that change to the owner.
- replaced: P2 also judging what the plan makes untrue, what it leaves unrecorded, each entry of `tree_edits` and any change to the tree that no one was asked about, was on main; the owner changed it, because the plan asks, the implementation writes the tree, and the review of the change checks it.

### decision both-plan-reviews-review-every-plan
P1 and P2 review every plan before it is built, on each version of it, whether the research or the deep research wrote it.
- replaced: Running P1 only after the deep research, and P2 only when the plan changed the tree, was on main; the owner changed it, because then nobody reviewed a plan that had neither against the tree's decisions before its code was reviewed.
- rejected: the owner proposed that P2 be run by `tree_edits` alone; the owner rejected it, because a plan that changes no entry of the tree may still need its review, such as one that goes against a decision.

### decision the-plan-reviews-are-split-by-their-yardstick
P1 judges the plan by what `background` asks, the engineering rules and where it meets the works under way; P2 judges it against the design tree, which only P2 does; P1 answers its points in its why.
- rejected: an agent proposed one cell reviewing a plan against `background`, the engineering rules and the tree together; the owner rejected it, because a cell that does more does worse, all the more when it need not answer each point.
- replaced: P1's point on a decision the plan goes against that no entry covers was on main; the owner changed it, because one thing is judged in one place, and judging the plan against the tree is P2's job.
- rejected: an agent proposed that both reviews judge the plan against the tree after the deep research, accepting that they overlap; the owner rejected it, because the work of the two reviews was to be split plainly.

### decision the-plan-review-is-given-the-works-under-way
The engine writes the files each work of the project under way has changed and its plan names as a list, `parallel`, which P1 reads; the plan's writers name the planned files beside the plan.
- rejected: an agent proposed no list across works: each work records its own changed files and the review puts them together by `query`; the owner rejected it, because every review would spend queries each round and get later, less complete information.
