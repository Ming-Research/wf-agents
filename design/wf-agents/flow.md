- The flow puts guard rails on the models. A model is reliable most of the time, and when it is not it also makes very
  basic mistakes.

- There is one flow, compiled into the program. Every project runs it and it says nothing about any project, whose
  differences are all in its configuration. The same flow must run any project, wf-agents included. A flow for each
  project would bring switches on the page, batch updates of near-identical flows and versions that branch.

- The flow is compiled before it runs. The compiler walks every state the engine can be in: every cell is reached, every
  return takes exactly one row, every recorded value is read, every loop passes a count or a wait, every input has a
  source. A flow that fails this never reaches a running work, since finding the fault halfway through a work costs far
  more than a failure at compile time.

- The flow has three graphs, the engineer's, the report's and the architect's, which meet only through the database:
  what the hub sends the architect from an engineer enters the architect's queue, and the architect acts on works
  through its returns. One graph, or a row from one graph into another, was not taken: an architect runs for a long time
  over every work of its project, while each work runs on its own, and the compiler can then check each graph apart.

- A work that delivers only a report runs a graph of its own: the step that brings its history and one research, both as
  an engineer's work has them, then the full CI, the merge, the end. A report settles how to do something very uncertain
  at first, or whether to do it at all; the architect creates an item for one, for an item that uncertain or for what
  the owner wants studied. What a work delivers is set when its item is made, unlike how hard it is, which shows only as
  it is done, so this is no light road beside a heavy one. A conclusion that decides the project's design goes to the
  owner as a change to the tree, which, once the owner says yes, goes in with the report as an engineer's does. Beyond
  that it changes only the research folder, which conflicts with nothing, so it needs no implementation, review or merge
  queue, though it waits at the merge gate as every work does. Run through the engineer's graph, it took every cell made
  for a change to the code (Article 3).

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

- To understand something, an agent reads both the code and the design tree's decisions on it: the code holds what is
  done, the tree why it is so and where it is going, ahead of the code. Either alone misleads: the code gives no
  reasons, and the tree does not say how far the code has got (Article 2).

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

- What a review will check, the cell that does the work is told first and, where it applies, hands in done, such as a
  measurement of a change's effect on performance, or a defect's like cases and what keeps it from coming back: the
  review then finds what was told and not done, and is not the first to ask. A review that is the first to ask for
  something sends the work back a round every time, for what the agent would have done in the round before had it been
  told (Article 3).

- The flow's design must converge, getting nearer the goal with each polish, and needs a way to model this complexity
  rather than going through the cases cell by cell. The possibilities diverge as cells grow, and considering each case
  of each cell soon grows past handling.

- Going through the flow or adding to it uses one standard. A problem is first placed: whether every work meets it or
  only a kind of work, and whether that kind is what the cell it would go in serves; a cell that serves every work
  demands of none an answer only some works have, such as whether a defect was seen to happen or whether the work should
  be split. What is to be stopped is then mapped onto a means that can be used, weighed against its cost: a line of a
  prompt costs little, a question an answer from every work it is asked of, a cell a round in every work that passes it;
  one is taken only where what it stops is worth that (Articles 3 and 4). Its consequences are looked at too: whether it
  makes small things large or large things small, or leaves an agent unable to do what it could, or forced to go around;
  they are worse than flow bugs such as an endless loop. Most cells do stop errors and what lacked was one standard;
  additions made where a problem showed, each reasonable alone, together made most of the flow's problems.

- A problem is solved first by writing it in a cell's input, and only if that is not enough by a tool of the flow: what
  prompting alone cannot do, needing another pair of eyes, a CI check, adversarial testing an agent cannot do for
  itself, or what it does badly even when told. One line of prompt settles most things. Mechanical checks and another
  pair of eyes cost much, and with rules alone a model keeps only some of them.

- A cell's prompt, inputs or outputs change only to solve a problem that can be named, and only after the changed cell
  has run on several real past cases and its inputs and returns have been set beside those of the cell as it was, case
  for case; a part of the flow with no serious problem is left as it is. Changes reasoned out alone left cells worse
  than before, and a change to a cell that works risks what it already does well (Article 4: what solves no named
  problem is not added). Judgment comes from what the cell really returns (Article 2).

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
