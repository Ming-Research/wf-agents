- A cell's inputs are of two kinds: always, given every time; and conditional, brought along if the session lacks them
  and only referred to if it has them. They are not sorted by origin, and none is given only at the start. A session may
  lack something for being new, for having changed context on filling up, or for having changed agent, and this division
  lets the context change midway. A separate kind for what the agent reads itself differs nothing from always, and a
  division by origin means nothing, since what an agent wrote itself is in the old session and not the new.

- What the engine gives an agent from a project's repository, its constitution and its AGENTS.md, is read as the
  project's main has it at one commit pinned for the round, never from a worktree's files, so that nothing a work's
  branch or worktree holds changes it. The cell that consults the design tree (flow.md, "A cheap model's cell finds and
  reads the decisions") reads, for an engineer, its work's branch instead, since the changes to the tree this work has
  already written for its design cards must be seen by the cells after it.

- Every round tells the agent what this round is for: its title, why, and what to hand in. The task's full text, like
  the rules, is given only as an address once the session has it. The next round's input is all the engine gives and the
  only thing that steers a session, and without this the agent does not know what the round is for. A session per round
  with everything given is plainly not feasible.

- An agent called in, such as an advisor, a reviewer or the architect, is not on the engineer's line; what it needs from
  that line it gets as named inputs, by the rule for all lines. Its session has not seen what the engineer did.

- No input copies all rounds' inputs and outputs to an agent as history. The general rule already delivers what each
  cell needs, and more only repeats it and brings in the engine cells' inputs as well.

- A prompt has no warnings against what an agent would not do anyway. Left unsaid it would not do it, and saying it
  wastes tokens.

- The flow sends no list of items anywhere: an agent that wants to see them uses the query tool, whose description must
  serve every cell. More than one cell looks at items, and taking the list from the planning cell alone leaves forks
  without the base's copy, so that the engine sends them the whole, which is worse.
