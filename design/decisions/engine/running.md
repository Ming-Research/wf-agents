# The engine's process

How the engine runs on a machine: its own process, where it keeps the works' worktrees, and the agents' programs it starts.

### decision one-process-serves-everything
The engine, its interface and the owner's page are served by one process.
- rejected: an agent proposed a separate process that serves the page and meets the engine only through the store; the owner rejected it, because the reasons for splitting them did not hold, and one process answers the owner at once and tells the page directly what happens.

### decision a-new-build-is-installed-by-hand
A new build of the engine is installed and started by hand: the engine never installs a build or restarts itself.
- rejected: an agent proposed a deployment in which CI builds a package after a merge and the engine restarts on the new package when no round is running; the owner rejected it, because changing the engine by hand is fine once the system improves itself and passes acceptance on the test system.
- rejected: an agent proposed that the engine install itself and restart on each green commit of main, as wf-board's deploy agent did; the owner rejected it, because a restart holds every work until the owner looks, so a redeploy at each merge would stop the night's works.

### decision a-works-worktree-is-in-the-data-directory
A work's worktree is made in the data directory, under `worktrees/`, on a branch of the work's own.
- rejected: an agent proposed putting the works' worktrees under the directory of the wf projects; the owner rejected it, because that directory belongs to wf, and wf-agents' worktrees go under its own data directory, which git keeps with the worktrees ignored.
- rejected: an agent proposed keeping the worktrees, the logs and the tools' output apart from the data, as a runtime that can be thrown away; the owner rejected it, because what cannot be thrown away belongs in the data.

### decision agents-run-by-their-programs-print-mode
An agent's round is one run of its program's print mode, `claude -p` unless the installation names another program.
- rejected: an agent proposed running the agents through the Claude Agent SDK; the owner rejected it, because with a print mode he can switch accounts in the terminal or move to another program, since many programs have one, and the SDK showed no advantage that a print mode lacks.

### decision the-programs-own-system-prompt-stays
The agents' program keeps its own system prompt, with the line's system prompt appended after it, and keeps its tool for sub-agents.
- rejected: an agent proposed loading only the tools the agents use and dropping the program's own system prompt and its sub-agent tool; the owner rejected it, because he wanted to keep the prompt, and leaned toward keeping the sub-agents, though he was unsure about them.

### decision the-agents-program-streams-its-input-and-output
The agents' program takes its message as a stream of JSON and gives its output as a stream of JSON too, one line for each thing it does.
- replaced: One result given at the end of the round was on main; the owner changed it, because the old engine streamed the output and so showed what an agent is doing, and that is to be shown again.
- replaced: Giving the message as plain input was on main; the owner changed it, because the old engine streamed it, and a stream can carry pictures.

### decision tool-inputs-are-checked-before-they-stream
The engine runs the agents' program with `CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING` set to 0, so that the service checks each tool's input as JSON before it streams it.
- replaced: Leaving the program to stream tool inputs as they are made was on main; the owner changed it, because it matters to mend the returns that came back cut off, and with that streaming the service no longer checks a tool's input.

### decision an-agent-ends-with-the-engine
An agent's program, like every program the engine runs, is stopped when the engine's process ends, however it ends, by a small shell in its own process group that waits on a pipe the engine holds.
- replaced: Letting the agents' programs outlive the engine, with the next engine stopping them before it runs a round, was on main; the owner changed it, because a program left running spends and changes the worktree unseen: one ran eight seconds after the engine ended and committed.

### decision the-programs-directory-is-the-engines
The agents' program keeps its settings and sessions in the engine's `profile/` in the data directory, and every command that runs it tells it to keep a session for a thousand years.
- replaced: A directory for the program named by the installation's `agents.config_dir`, under the data directory's `agents/`, was on main; the owner changed it, because he did not want to keep that `agents` directory.
- replaced: The program's own cleanup, deleting a session after thirty days, was on main; the owner changed it, because it deleted the agents' transcripts, which he wanted kept.

### decision agents-run-as-the-engines-user
The agents' program runs as the engine's own system user, and nothing the engine does keeps it out of the data directory; only the program's own rules do.
- rejected: an agent proposed another system user for the agents, or else refusing the data directory to their file tools by path and dropping the shell commands that read files; the owner rejected it, because an agent works in its own worktree, and only some harmless commands were to be allowed.
