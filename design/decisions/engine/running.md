# The engine's process

How the engine runs on a machine: its own process, where it keeps the works' worktrees, and the agents' programs it starts.

### decision one-process-serves-everything
The engine, its interface and the owner's page are served by one process.
- rejected: an agent proposed a process of its own serving the page, meeting the engine only through the store; the owner rejected it, because the reasons for splitting them did not hold, and one process answers the owner at once and tells the page straight what happens.

### decision a-new-build-is-installed-by-hand
A new build of the engine is installed and started by hand: the engine never installs a build or restarts itself.
- rejected: an agent proposed a deployment in which CI builds a package after a merge and the engine restarts on it when no round runs; the owner rejected it, because changing the engine by hand is fine once the system improves itself and passes acceptance on the test system.
- rejected: an agent proposed that the engine install itself and restart on each green commit of main, as wf-board's deploy agent did; the owner rejected it, because a restart holds every work until the owner looks, so a redeploy at each merge would stop the night's works.

### decision a-works-worktree-is-in-the-data-directory
A work's worktree is made in the data directory, under `worktrees/`, on a branch of the work's own.
- rejected: an agent proposed the works' worktrees under the directory of the wf projects; the owner rejected it, because that place is wf's, and wf-agents' worktrees go under its data directory, which git keeps with the worktrees ignored.
- rejected: an agent proposed keeping the worktrees, the logs and the tools' output apart from the data, as a runtime that may be thrown away; the owner rejected it, because what cannot be thrown away belongs in the data.

### decision agents-run-by-their-programs-print-mode
An agent's round is one run of its program's print mode, `claude -p` unless the installation names another program.
- rejected: an agent proposed running the agents through the Claude Agent SDK; the owner rejected it, because with a print mode he can switch accounts in the terminal or move to another program, as many have one, and the SDK showed no advantage it lacks.

### decision the-programs-own-system-prompt-stays
The agents' program keeps its own system prompt, the line's appended after it, and its tool for sub-agents.
- rejected: an agent proposed loading only the tools the agents use and dropping the program's own system prompt and its sub-agent tool; the owner rejected it, because he would keep the prompt, and leaned to keeping the sub-agents, though unsure of them.

### decision the-agents-program-streams-its-input-and-output
The agents' program takes its message as a stream of JSON and gives its output as one, a line for each thing it does.
- replaced: One result given as the round ends stood on main; the owner changed it, because the old engine streamed it, and what an agent is doing was shown so and is to be shown again.
- replaced: The message given as plain input stood on main; the owner changed it, because the old engine streamed it, which can carry pictures.

### decision tool-inputs-are-checked-before-they-stream
The agents' program is run with the service checking each tool's input as JSON before it streams it, `CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING` set to 0.
- replaced: Leaving the program to stream tool inputs as they are made stood on main; the owner changed it, because mending the returns that came cut off matters, and with that streaming the service no longer checks a tool's input.

### decision no-toolchain-is-installed-in-a-round
The agents' program is run with `RUSTUP_AUTO_INSTALL` set to 0, so that no toolchain is installed within a round.
- replaced: Leaving the variable unset stood on main; the owner changed it, because a round then downloads no hundreds of megabytes unseen, and a missing toolchain is an error the agent sees.

### decision an-agent-ends-with-the-engine
An agent's program, as every program the engine runs, is stopped as the engine's process ends, however it ends, by a small shell in a group of its own that waits on a pipe the engine holds.
- replaced: The agents' programs outliving the engine, the next engine stopping the last one's before it runs a round, stood on main; the owner changed it, because a program left running spends and changes the worktree unseen: one ran eight seconds past the engine and committed.

### decision the-programs-directory-is-the-engines
The agents' program keeps its settings and sessions in the engine's `profile/` of the data directory, and every command of it is told to keep a session a thousand years.
- replaced: The program's directory named by the installation's `agents.config_dir`, under the data directory's `agents/`, stood on main; the owner changed it, because he did not want that `agents` directory to stay.
- replaced: The program's own cleanup, deleting a session after thirty days, stood on main; the owner changed it, because it deleted the agents' transcripts, which he wanted kept.

### decision agents-run-as-the-engines-user
The agents' program runs under the engine's own system user, and nothing of the engine's keeps it from the data directory but the program's own rules.
- rejected: an agent proposed another system user for the agents, or the data directory refused to their file tools by path and the shell's commands that read files dropped; the owner rejected it, because an agent works in its own worktree, and only some harmless commands were to be opened.
