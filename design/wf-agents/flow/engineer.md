- The flow bends to the work: a fix of a few lines goes through in minutes, and a heavy matter takes the full design.
  Two hours for each work is too much, and a fix of a few lines that takes an hour of flow and four or five hours with
  the queue is more than the owner can wait. The heavier the flow, the more it rests on the architect writing items
  well.

- How heavy a work runs is decided as it is done, by how hard it really is, and the architect marks nothing beforehand.
  Size and nature are tangled: a defect and a need come mixed, where the defect must be fixed first to build on, and
  what looks simple can turn serious halfway. A mark by the architect on creating an item was tried: almost no item got
  it and all took the heavy way. A separate light flow for bug fixes and small features was rejected, since flexible
  does not mean one heavy road and one light; so was a half-made short cut in the engineer's graph, since what is in the
  flow must be as good as it can be.

- The engineer's flow has no planning stages or budget, no cell that checks a plan or a budget, and no question of going
  over plan. A change the design tree needs goes on a design card (flow/design-cards.md, "A technical choice for the
  owner"), spending has a ceiling and a page of its own, and whether the direction is off is for the architect to judge
  under the policy.

- Running everything for every work, works fighting over CI, dependencies stretched out in time and retesting after
  merge conflicts are solved in the flow, not treated as problems of version control. Works differ in size and in how
  much they need testing, and this pattern, taken from human development, does not fit AI development.

- Cell by cell, the flow makes sure the engineer does not miss the key steps; one free cell with a page of instructions
  is not enough. Given a free cell, engineers often forget or skip key steps, do not read the code or understand the
  problem well, and are too cautious one moment and too loose the next.

- The acceptance may change during a work. Locking it after the first design was too strict.

- Research does not only reason down from the decisions in force: where one must change or the work cannot be done, it
  proposes changing it (flow/design-cards.md, "A technical choice for the owner"). Reasoning down from a decision cannot
  find that.

- The implementation and the fix keep the design true instead of declaring departures from it: a decision the design and
  the tree do not record is written into the design's passage with why, a design wrong in its direction goes back to
  research, and a workaround for a problem outside the work is reported and marked in the code. What they change in the
  design they change in place, so that it reads as one design. A departure only declared, left for the review to
  challenge, was one the agent knew of and neither fixed nor recorded, and cost the review a round (Article 3). The code
  review judges each passage so written as a design is reviewed, not only as the standard it checks the code against.

- A defect found in passing, or something that should be improved, is reported only when it is worth a real item. The
  architect's time is to be used sparingly, and every small matter reported takes some of it.
