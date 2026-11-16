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

### decision an-agent-ends-with-the-engine
An agent's program, as every program the engine runs in a group of processes of its own, is stopped as the engine's
process ends, however it ends, by a small shell in a group of its own that waits on a pipe only that process holds; the
shell is let go of only where the program's output ends with the engine there to see it, a program whose shell cannot
be started is stopped at once and its run failed, and an agent's group left all the same, as where its shell was ended
before the engine's process, is stopped before its round is run again.
Why: Left to run with no engine, a program spends and changes the worktree where nothing guards, counts or stops it: in
work 4's trial the implementation's went on eight seconds, four calls, three edits and a commit nobody saw, its return
lost (principle no-unseen-spending-loop); met by the next engine running its round again, it would be two agents in one
worktree (decision one-engine-on-a-data-directory). The shell's group is its own so that the terminal's signals to the
engine's group, Ctrl-C's or a closed terminal's, do not end it with the engine. A signal handler of the engine's was
turned down, as it sees no crash nor `kill -9`; so was letting the shell go whenever the engine drops it, as an engine
that ends by itself drops the rounds it runs as it ends; and so was running a program with no shell, which would be
the fault guarded against, seen by nobody (principle nothing-passes-for-normal).

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
