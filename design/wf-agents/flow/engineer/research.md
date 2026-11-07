- Before the first research, the deep research and the implementation each begin, the work's branch is brought up to the
  latest main, so that no stage works on old code. A stage whose session goes on is given the files main changed, since
  the session remembers the old ones and, listed, knows to read them again; a stage in a fresh session reads the code as
  it now is and is given none.

- The first research may use CI and the tools. Without running things it cannot find a root cause.

- Research is held to researching by what it is asked, not by where it may write. What it makes for the problem, data,
  code, probes and notes, goes in the research folder and is not left about elsewhere; a test that reproduces the
  problem goes where the project's tests are, so that it runs where they run and the implementation keeps it. A simple
  research leaves nothing but its conclusion, and a conclusion that sets a rule beyond this work goes on a design card
  (flow/design-cards.md, "A card is put only where"). A limit of everything to the research folder, set to keep research
  on its task, also kept a reproduction from running where the tests run, so a defect could not be seen to happen before
  it was fixed.

- Where the first research can settle the way, it writes how to change it and goes straight to implementing; only where
  it cannot does deep research follow. Most works settle there, and what needs more is a hard performance problem, a
  feature to design or a stubborn defect. Two further cells asking for the root cause and whether it is a design flaw
  were rejected: asked of most works, such as a missing line of CSS, they make no sense, and they overlap deep research.

- Only a work that really needs it takes deep research, never for the sake of an approval. Putting a thing up for
  approval is enough, and a trip through deep research for it is waste.

- The first research has a way to stop rather than running on to a forced conclusion, and whether deep research follows
  is decided by a principle the engineer can understand, not by a hard measure. Even a forced first research has a
  conclusion, which it may not see does not hold, and a hard measure misses many cases.

- In doubt, the first research looks further rather than stopping to ask, and asks only at a defect it cannot see
  happen, a real choice going on a design card (flow/design-cards.md, "A technical choice for the owner"). Asking the
  architect brings little more, and stopping costs much.

- A defect takes no design: the effort goes to how it came about, how to prevent it, whether the fix is sound, and
  whether a flaw in the architecture caused it. A design is for setting a route for an uncertain need. How it came about
  is traced to where it began and written in what the research hands on, since a defect understood at its source is
  plainly fixed; it goes into no message of its own, since most defects are cases no test reached, which no step of the
  flow would have stopped.

- How deep research goes depends on the work: where a change reaches widely the agent first understands the system, and
  a very local one may be changed with no look at the architecture. An agent has no memory of the owner's intent or of
  what earlier agents thought, and changing things at once may break them; a local matter is not worth reading the
  architecture for.

- A defect is fixed only after it has been seen to happen where it shows, as on the page in a real browser or in the
  program on its own data. Where no test can be written for it, as for something that shows only now and then, none is
  forced: a probe, a log or the owner's own run may show it instead. A defect still seen nowhere after looking as far as
  the tools reach, whether the tools cannot reach where it shows or the state has since changed, stops the work at the
  research it is in, with no deep research after it, by an ask of the owner of how to go on, as what only a person can
  solve. Whether the tools fall short is not known then, and deep research has the same tools. A cause inferred and
  fixed once passed its test in a stand-in for the browser while the defect stayed, and came back as a new item (Article
  2).

- When a work took over something half done, the first research gets pointers to look with, sees how far it got and then
  researches as usual, and keeping or changing what is there is the engineer's judgment. Half-done work may come from
  another flow or another agent with no same chart to compare, so skipping cells mechanically, because a definition and
  its inputs did not change, goes wrong.

- Deep research uses a model one level above the item's, as the advisor does, and few works take that way. At the item's
  model a weaker agent may fix the surface and stop, hiding a larger problem, and what it overlooks the flow cannot find
  again elsewhere.

- A design does not first make up several ways to compare: it understands the decisions above it, thinks of what is
  reasonable for the problem, and compares only where another direction really exists. Some problems plainly have one
  reasonable way, and a made-up comparison is empty.

- When the design is done, it is looked at again for a better design (flow.md, "A solution chosen for a problem must
  itself be challenged"): another way to the same end, or the same end reached more simply, at less cost or with less to
  keep up, including a way the design tree rules out. Where one is worth its cost, the design is written again, whole,
  in place of the first; otherwise the first stands. Pressing instead on where the design was not thought through rarely
  broke a premise (two proposals in eighty-six), while asking for a better design found one in most designs.

- After deep research there are no fork questions to answer one by one: its departures from the decisions in force are
  on its design cards (flow/design-cards.md, "A technical choice for the owner"), and the rest is left to the review.

- A design too large for one work, falling into parts that can each be done and merged on their own, leaves deep
  research by a way of its own: the engine puts the split to the architect through the hub and holds the work there
  until the architect makes the item a goal over the parts, which ends the work, keeps the first part for the work and
  makes the others items, or declines with why, which takes the work back to deep research. Splitting items is the
  architect's duty (flow/architect.md, "The architect's duty"), and a stop that a cell's output calls for is drawn as a
  way out, not left to an agent's ask (Article 7).
