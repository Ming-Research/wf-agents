- The flow bends to the work: a small fix runs through fast, and a heavy matter takes the full design. The heavier the
  flow, the more it rests on the architect writing items well.

- How heavy a work runs is decided as it is done, by how hard it really is, and the architect marks nothing beforehand.
  Size and nature are tangled: a defect and a need come mixed, where the defect must be fixed first to build on, and
  what looks simple can turn serious halfway. A mark by the architect on creating an item was tried: almost no item got
  it and all took the heavy way. A separate light flow for bug fixes and small features was rejected, since flexible
  does not mean one heavy road and one light; so was a half-made short cut in the engineer's graph, since what is in the
  flow must be as good as it can be.

- The engineer's flow has no planning stages, gates or budget, no cell that checks a plan or a budget, and no question
  of going over plan. A change to a specification, an interface or a format goes to the owner, spending has a ceiling
  and a page of its own, and whether the direction is off is for the architect to judge under the policy.

- Running everything for every work, works fighting over CI, dependencies stretched out in time and retesting after
  merge conflicts are solved in the flow, not treated as problems of version control. Works differ in size and in how
  much they need testing, and this pattern, taken from human development, does not fit AI development.

- The time a work takes fits the work: a fix of a few lines goes through in minutes. Two hours for each work is too
  much, and a fix of a few lines that takes an hour of flow and four or five hours with the queue is more than the owner
  can wait.

- Cell by cell, the flow makes sure the engineer does not miss the key steps; one free cell with a page of instructions
  is not enough. Given a free cell, engineers often forget or skip key steps, do not read the code or understand the
  problem well, and are too cautious one moment and too loose the next.

- A question that means nothing for most works, such as whether to split it, is not asked of every work. Most works
  almost surely need no split, and even a small fix would have had to answer.

- What needs approval is put up for approval and the work goes on; it stops only before the merge, if still not
  approved. Put up early, it leaves the owner time, and raised only before the merge it blocks the work; stopping at the
  design until it is reviewed makes every work wait longer. When the owner disagrees, the work goes back to the first
  research, which judges whether to do the deep research again. The price is that what was implemented by then may be
  thrown away.

- Options and doubts are written in the design itself, and the owner is asked to read the design. Taken out of it into a
  card of their own, they have no background, no progress and no place in the design, and the owner cannot follow them.

- The acceptance may change during a work. Locking it after the first design was too strict.

- Research does not only reason down from the decisions in force: where one must change or the work cannot be done, it
  proposes changing it. Reasoning down from a decision cannot find that.

- An engineer's question carries no category label, and the question itself is written clearly. Written clearly, the
  architect understands it with no label. A category for accepting slowness was odd, and about eight categories that
  change the handling were not needed.

- Side reports are of two kinds: a defect found in passing, and something unreasonable that should be improved. Each
  must be worth a real item, and what is not is not reported. The architect's time is to be used sparingly, and every
  small matter reported takes some of it.
