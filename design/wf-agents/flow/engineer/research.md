- Every work opens with a step that brings the material, the background and the decisions it touches, however small the
  work. Skipped for a small one, it can miss a decision that matters, and this step is how decisions reach the whole
  work.

- Before the first research, the deep research and the implementation each begin, the work's branch is brought up to the
  latest main, and that stage's input lists the files main changed. Otherwise the agent sees old code, and a session
  that goes on remembers the old files; listed, it knows to read them again.

- The first research may use CI and the tools. Without running things it cannot find a root cause.

- Where the first research can settle the way, it writes the change and goes straight to implementing; only where it
  cannot does deep research follow. Most works settle there, and what needs more is a hard performance problem, a
  feature to design or a stubborn defect. Two further cells asking for the root cause and whether it is a design flaw
  were rejected: asked of most works, such as a missing line of CSS, they make no sense, and they overlap deep research.

- Only a work that really needs it takes deep research, never for the sake of an approval. Putting a thing up for
  approval is enough, and a trip through deep research for it is waste.

- The first research has a way to stop rather than running on to a forced conclusion, and whether deep research follows
  is decided by a principle the engineer can understand, not by a hard measure. Even a forced first research has a
  conclusion, which it may not see does not hold, and a hard measure misses many cases.

- In doubt, the first research looks further rather than stopping to ask, and only at a real trade-off does it go to
  someone. Asking the architect brings little more, and stopping costs much.

- A defect takes no design: the effort goes to how it came about, how to prevent it, whether the fix is sound, and
  whether a flaw in the architecture caused it. A design is for setting a route for an uncertain need.

- How deep research goes depends on the work: where a change reaches widely the agent first understands the system, and
  a very local one may be changed with no look at the architecture. An agent has no memory of the owner's intent or of
  what earlier agents thought, and changing things at once may break them; a local matter is not worth reading the
  architecture for.

- Where no reproducing test can be written, as for something that shows only now and then on the page, the work does not
  get stuck on it: it studies the code or proves it another way. Forcing a test that cannot be written is no use, and
  there are many such cases.

- A work that takes over something half done takes the branch, the worktree and the pull request of the work before it,
  and only without one starts a branch from main. The first research gets pointers to look with, sees how far it got and
  then researches as usual, and keeping or changing what is there is the engineer's judgment. Half-done work may come
  from another flow or another agent with no same chart to compare, so skipping cells mechanically, because a definition
  and its inputs did not change, goes wrong.

- Deep research uses a model one level above the item's, as the advisor does, and few works take that way. At the item's
  model a weaker agent may fix the surface and stop, hiding a larger problem, and what it overlooks the flow cannot find
  again elsewhere.

- A design does not first make up several ways to compare: it understands the decisions above it, thinks of what is
  reasonable for the problem, and compares only where another direction really exists. Some problems plainly have one
  reasonable way, and a made-up comparison is empty.

- When the design is done it is thought through again, not by asking whether something is better but by pressing where
  it was not thought through: whether a design built on past decisions is bound by them and can break out. Only after
  doing it once, and asked so, does a better way come.

- After deep research there are no fork questions to answer one by one: a departure from a decision in force is said in
  the prompt, and the rest is left to the review, since the owner reviews the design.
