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

### decision each-tool-input-comes-whole
The agents' program is run with each tool's input streamed whole, not in pieces as the model writes it.
Why: Streamed in pieces, an input's JSON is not checked by the service, and returns came cut off; streamed whole, they
stopped coming so, as the old engine found (not measured here).

### decision no-toolchain-is-installed-for-an-agent
The agents' program is run with rustup's installing of a missing toolchain turned off: a command an agent runs that
asks for a toolchain the machine lacks fails.
Why: Installing it downloads and writes beyond the worktree, which every agent is refused (principle
refuse-as-little-as-must-be).

### decision the-owners-words-wait-beside-the-round
What the owner says to a round under way reaches its session through a hook of the agents' program after a use of a
tool, which moves aside a file the runner put among the round's files and prints it; after the tool that makes the
return, it gives nothing.
Why: A hook that asked the engine would work only where the program runs on the engine's machine. Given after the
return is made, the words would be taken as delivered and never read.
