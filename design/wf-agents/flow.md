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
  an engineer's work has them, then a review of its change to the design tree when it makes one, the full CI, the merge,
  the end. A report settles how to do something very uncertain at first, or whether to do it at all; the architect
  creates an item for one, for an item that uncertain or for what the owner wants studied. What a work delivers is set
  when its item is made, unlike how hard it is, which shows only as it is done, so this is no light road beside a heavy
  one. A conclusion that decides the project's design goes on a design card (flow/design-cards.md, "A technical choice
  for the owner"); the research writes the change of the option chosen, and the review, in a session of its own, checks
  it against the whole tree. Beyond that it changes only the research folder, which conflicts with nothing, so it needs
  no implementation, code review or merge queue, though it waits at the merge gate as every work does. Run through the
  engineer's graph, it took every cell made for a change to the code (Article 3).

- Where an agent decides which way a work goes, each fork is one yes/no question asked by the cell just before it, and
  the way on depends on that answer alone, with no labels to route on later. A label compresses the information, so that
  judging by it later distorts, and it hides steps no one can see. "When unsure, take the heavier way" is not used: the
  heavier way costs much, so the question is answered as the facts stand. Where an ask holds its work is no fork: the
  work's way stays the same, and only when it may go on changes.

- What an engineer puts to the owner, an ask or a design card, stands in the item where it belongs, in its design where
  it has one, with the background written around it; the owner sees by the marked passages where it changed and where a
  question stands. Taken out on its own, a question had no background and no place in the design.

- The owner is shown one count of what waits on them, the asks to the owner or the flow's keeper holding a work or a
  matter now and the design cards holding a work, and everything else as unread; only the first notifies, as each comes,
  and once an hour while any stays. A design card the work went on past shows as to be confirmed, and reminds of nothing
  until the merge waits for it. Several counts with different meanings disagreed and were not trusted.

- Cells share tools and output forms wherever they can, with no second copy for the same thing. A cell may have an
  ability of its own, never a second one of what another cell has. Every cell of a line carries the line's whole schema
  in every round, so what one cell writes for itself enlarges the input of all.

- A cell that continues another's session has what that cell found, so what the earlier cell hands on is only what a
  cell outside the session or the owner reads, and nothing is written twice for the one that follows. Every way the work
  can go keeps it so: where the session may not continue, as when it is restarted for its size or the next cell runs in
  a session of its own, that cell is given what the earlier one wrote (flow/inputs.md, "A cell's inputs are of two
  kinds"), so a restart loses no more than what was never written. Output costs the most of a round, and every field
  rides in every round of its line (flow.md, "Cells share tools and output forms wherever they can").

- The flow's definition holds what agents are told and what the chart shows of what the flow does; why it is so is in
  the design tree, and an instruction does not carry its reasons. Text that reaches no agent and shows on no chart is
  not kept, and the compiler refuses it, as it refuses a field nothing reads. Reasons kept in the definition were sent
  to agents every round and drifted from the tree's.

- A cell keeps no state: a stage is not made a small state machine that stops in several places and pieces state
  together by various means, and such a problem is met by changing the state machine itself. Greying a button or an
  input while a talk goes on only goes around it, and what is unreasonable in the machine remains.

- A cheap model's cell finds and reads the decisions a question touches, for an engineer, an architect or the assistant,
  quoting them and judging nothing. Reading the whole tree each time costs too much, and a model that judged would put
  its reading in place of the decisions.

- To understand something, an agent reads both the code and the design tree's decisions on it: the code holds what is
  done, the tree why it is so and where it is going, ahead of the code. Either alone misleads: the code gives no
  reasons, and the tree does not say how far the code has got (Article 2).

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

- The flow is audited cell by cell from time to time, the agent cells above all, starting from the problem each cell
  controls and the ideal cell, the cell as it is and its recorded rounds serving only as evidence. Each input is judged
  for its order (wf-agents.md, "What an agent is sent is ordered by how widely it is the same"), for whether a session
  is sent again what it has (flow/inputs.md, "A cell's inputs are of two kinds"), its tokens, its content and size, and
  whether everything the cell needs is there and nothing else; each output for whether it does what the inputs mean it
  to and has a reader (flow/returns.md, "Every field of a return must have a reader"). The evidence is the recorded
  rounds, with each input's size, the cache read and written, what the agents' reports on the flow asked for and the
  returns sent back, and runs of the changed cell on made inputs as well as real past ones (flow.md, "A cell's prompt,
  inputs or outputs change only to solve a problem that can be named"). Every cell is settled before any is changed and
  the settled cells are made into a new definition; a cell is fixed only once the owner has reviewed it there, on its
  chart, and changes after that only by the owner's yes. Cells added one at a time, each reasonable alone, drifted
  apart, and only a pass over all of them showed it; and cells settled in drafts and fixed there came out otherwise in
  the definition made from them.

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
