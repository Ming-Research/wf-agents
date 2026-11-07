- The flow puts guard rails on the models. A model is reliable most of the time, and when it is not it also makes very
  basic mistakes.

- There is one flow, compiled into the program. Every project runs it and it says nothing about any project, whose
  differences are all in its configuration. The same flow must run any project, wf-agents included. A flow for each
  project would bring switches on the page, batch updates of near-identical flows and versions that branch.

- The flow is compiled before it runs. The compiler walks every state the engine can be in: every cell is reached, every
  return takes exactly one row, every recorded value is read, every loop passes a count or a wait, every input has a
  source. A flow that fails this never reaches a running work, since finding the fault halfway through a work costs far
  more than a failure at compile time.

- The flow has two graphs, the engineer's and the architect's, which meet only through the database: what the hub sends
  the architect from an engineer enters the architect's queue, and the architect acts on works through its returns. One
  graph, or a row from one graph into the other, was not taken: an architect runs for a long time over every work of its
  project, while each work runs on its own, and the compiler can then check each graph apart.

- When the flow changes, a running work neither ends nor starts over: it goes on where it stands if it can, goes back to
  the cell it must do again if not, and where it cannot go back, as past a merge, it stops for the flow's keeper.
  Starting over wastes what was done, and holding a work in a state the new flow lacks goes wrong. A work whose graph
  the new flow lacks altogether still ends.

- Where an agent decides which way a work goes, each fork is one yes/no question asked by the cell just before it, and
  the way on depends on that answer alone, with no labels to route on later. A label compresses the information, so that
  judging by it later distorts, and it hides steps no one can see. "When unsure, take the heavier way" is not used: the
  heavier way costs much, so the question is answered as the facts stand. Where an ask holds its work is no fork: the
  work's way stays the same, and only when it may go on changes.

- Cells share tools and output forms wherever they can, with no second copy for the same thing, and every lookup in the
  database goes through one general query tool. A cell may have an ability of its own, never a second one of what
  another cell has. Every cell of a line carries the line's whole schema in every round, so what one cell writes for
  itself enlarges the input of all. A tool for each kind of lookup was dropped: what such tools read, the general query
  and git read too, and each was one more piece of schema.

- A cheap model's cell finds and reads the decisions a question touches, for an engineer, an architect or the assistant,
  quoting them and judging nothing. Reading the whole tree each time costs too much, and a model that judged would put
  its reading in place of the decisions.

- The flow's definition holds what agents are told and what the chart shows of what the flow does; why it is so is in
  the design tree, and an instruction does not carry its reasons. Text that reaches no agent and shows on no chart is
  not kept, and the compiler refuses it, as it refuses a field nothing reads. Reasons kept in the definition were sent
  to agents every round and drifted from the tree's.

- What an agent writes for the owner to read is in the owner's language, as the installation sets it, and English where
  it sets none, with technical terms in their English original. The owner reads it to judge.

- The language a field of a return is written in is said in the return's schema and in no rule, since an agent writes a
  field by its description.

- A choice of the way the engine makes for the flow is drawn as a branch cell on the chart; no condition hangs on a cell
  or a row to make a way vanish in some cases. A way resting on a condition cannot be seen on the chart, and is a hidden
  switch.

- The section on what happens every round holds only what the engine really does on receiving a return, each with when
  it fires. Which view reads which field is how the database is queried, not what the engine does.

- A cell keeps no state: a stage is not made a small state machine that stops in several places and pieces state
  together by various means, and such a problem is met by changing the state machine itself. Greying a button or an
  input while a talk goes on only goes around it, and what is unreasonable in the machine remains.

- Every review can be disputed, and a dispute that holds goes up. A review is not always right.

- The flow's design must converge, getting nearer the goal with each polish, and needs a way to model this complexity
  rather than going through the cases cell by cell. The possibilities diverge as cells grow, and considering each case
  of each cell soon grows past handling.

- Going through the flow or adding a cell uses one standard: map what is to be stopped onto a tool that can be used, and
  look at the consequences, whether it makes small things large or large things small, or leaves an agent unable to do
  what it could, or forced to go around. Most cells do stop errors and what lacked was one standard, and such
  consequences are worse than flow bugs such as an endless loop.

- A problem is solved first by writing it in a cell's input, and only if that is not enough by a tool of the flow: what
  prompting alone cannot do, needing another pair of eyes, a CI check, adversarial testing an agent cannot do for
  itself, or what it does badly even when told. One line of prompt settles most things. Mechanical checks and another
  pair of eyes cost much, and with rules alone a model keeps only some of them.

- A solution chosen for a problem must itself be challenged: is this the fittest way, is there a better? This reasoning
  matters more than listing. Without it, the work goes back to solving problems while making others, with no
  convergence.

- A judgment needing research and focus, such as whether something is a design flaw, is asked as a step of its own, not
  as a side task in another prompt, and not every time; this forbids no agent elsewhere from making it. Asked in passing
  it is not thought through. Telling others only to report signs and not to judge limits them for no reason.

- A problem costly to guard against, which may stop happening as models get stronger, must be weighed for whether it
  deserves a guard rail. Concluding without reading the code is one, and the flow does not check for it. Models keep
  improving, and such guards weaken with time.

- Each cell says what problem it solves, how, why by this tool and at what cost. That gives iteration its basis: taking
  an ability from a cell shows which problem must then be solved elsewhere, and a new problem can find its place and
  tool.

- The flow's tests are a formal matter, not one trial run. The flow only runs once live, and the next upgrade would
  again meet what never ran.
