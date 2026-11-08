- A running work can be stopped and started over by taking over: a new work takes its branch, working directory and pull
  request, starting a branch from main only where there is none, walks from the first cell, and is told what exists and
  the owner's answers and choices before. What was made is not wasted. Clearing everything would lose the old work, and
  if really needed the new work can rewrite it.

- Starting a work over does not wait for the round under way to hand in: the round is cut off, and the new work begins
  once its agent's process has ended, since what a round of a work being started over hands in is of no use.

- Stopping the engine on request dispatches nothing more and waits for the rounds under way, and a pause, of one work or
  of all, takes effect at the end of the round under way, since what a round under way has already spent is not thrown
  away. Ending the engine any other way, an interrupt or a closed terminal included, stops the agents it was running
  with it: left running, an agent would meet the next engine running its round again, two agents in one worktree, as
  with two engines on one data directory (backend/engine.md, "One engine runs on a data directory").

- One engine runs on a data directory: it takes an operating-system lock before it opens or writes anything, and the
  lock goes with its process however it ends. Two engines would take the same work, running its round twice, with two
  agents in one worktree and two pushes; and a lock held by the process leaves nothing to clear by hand after a crash or
  a kill.

- The engine's git runs no program that a configuration an agent can write names. The worktrees of a checkout share one
  git configuration, which an agent's own git can write, and git runs the programs it names as the engine's user with
  the engine's credentials. So the engine's git ignores every setting that names a program (hooks, the file-system
  monitor, signing, credential helpers and prompts, its ssh command, remote helpers) and reads no attributes at all,
  since an attribute names a filter or a driver by any word, which cannot be listed and shut one by one. A remote the
  engine reaches then needs no password, and what it checks out and merges has none of the project's end-of-line
  conversion, merge drivers or large-file filters; an agent's own git is not affected.

- The engine's git still reads the machine's global and system git files for what names no program, such as URL rewrites
  and proxies the owner keeps there, since reading neither would drop them. What those files, the repository's own
  attributes file and ssh's own configuration can still name is a known gap: a write to any of them is a write outside
  the agent's worktree.
