- An agent that must query the database or run a tool does so by returning, the engine running it, and going on from the
  last round, never in mid-round; such a return need not carry progress. To the model both are one tool call, with the
  same in its context and the same count, and querying mid-round saves about three seconds each.

- How to query is in the tool, and the database's whole structure is not put into an agent's context: at over 20K that
  is very costly.

- Every lookup in the database goes through one general query tool. A tool for each kind of lookup was dropped: what
  such tools read, the general query and git read too, and each was one more piece of schema.

- No tool opens a page in a browser or takes screenshots; searching and fetching the web are another matter. A project's
  page is tested with a browser in CI, and an agent needs none on its machine.

- An engineer can take generated files back from CI. Unable to compile locally and unable to get generated files right
  by hand, an item would stop to wait for someone.

- A command an agent needs that reaches the network or writes files beside its change to the code is run by the engine
  as a tool drawn on the flow, never in the agent's own shell: the project's configuration names the command, and the
  agent asks for the tool by name. Letting one exact command through is a feature of one agent's command line, and a
  command in an agent's shell is on no chart and in none of the engine's records. The price is one more round for each
  request, about five seconds at the median beside the command's own time. Rejected: the agent running the configured
  command itself; the engine fetching on every round whether needed or not; fetching one package at a time, which is
  tied to a language; and reading a package's repository or the machine's cache, which lack what is published or depend
  on the machine.

- The directory a command run for an agent fills must be one the project's git ignores, which the engine checks before
  running anything, refusing otherwise, so that what the command writes never shows as a change of the work.

- A command the engine runs for an agent passes no permission check of the agent's command line and runs as the engine,
  with its whole environment and credentials, over a worktree the agent wrote; so it only fetches or writes what it is
  for: it compiles nothing, runs no code of a package, writes only its own directory or the lock file, moves nothing the
  manifest does not ask for, and the engine sets no environment for it. What such a command can still take from an
  agent's files is a known gap.

- A configured command is split into words and run without a shell, in a process group of its own, under a time limit
  that stops it and all it started, leaving nothing running; one that needs a shell says so itself. The commands come
  from configuration and run with no one watching: through a shell an argument can be read as another command, and a
  command that hangs, or children it forked, would outlive it and stall the work.

- An agent that researches, designs, implements, reviews, tests or advises can read the source and the types of the
  dependencies that a lock file the project's configuration lists locks, as published at the locked version, without
  compiling anything: every cell that judges a design or a change must be able to read what a dependency does (Article
  2), rather than write it as inferred or put the question to the architect.

- A measurement times a command on the code compared, on a twin of it and on the code measured, in rotating pairs, and
  keeps the raw times: the machine's noise drifts over time, rotating pairs cancel the drift, and the raw times can be
  analysed again.
