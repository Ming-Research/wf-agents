# The engine's process

How the engine runs on a machine: its own process, and the agents' programs it starts.

### decision one-process-serves-everything
The engine, its interface and the owner's page are served by one process.
Why: What the owner does, such as answering a question, must wake the engine at once and not wait for it to look into
the database; what happens is told to the page straight from the engine; and there is one thing to start and to bring
up to date. With the page a process of its own, each of these goes the long way round.

### decision one-engine-on-a-data-directory
One engine runs on a data directory: before it opens the store or writes anything it takes a lock of the operating
system's, which goes with the process however it ends.
Why: Two engines would take up the same work, run its round twice with two agents in one worktree, and push twice. A
lock the process holds leaves nothing to clean up by hand after a crash or a kill.

### decision an-agent-left-running-is-stopped-first
An agent's program runs in a group of processes of its own and can outlive the engine; a group that an engine which
ended left running is stopped before its round is run again.
Why: Left to run, it would meet the next engine running its round again: two agents in one worktree, as with two
engines on one data directory (decision one-engine-on-a-data-directory).

### decision tool-inputs-are-checked-before-they-stream
The agents' program is run with the service checking each tool's input as JSON before it streams the input.
Why: An input the service did not check could come cut off, and a return cut off is no return.

### decision the-programs-own-memory-is-off
The agents' program is run with its own memory off, by the setting `autoMemoryEnabled` in the settings the engine
gives it.
Why: A session holds what the engine gives it and what its agent reads; the program's own memory would hold what the
engine never sees, and the program's own system prompt tells the agent to write there, against the rule the agents are
given to keep nothing there. Its minimal mode, `--bare`, which turns the memory off as well, was turned down: it turns
off the hooks the engine gives too.
