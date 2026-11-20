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
worktree (decision one-engine-on-a-data-directory). A signal handler of the engine's was turned down, as it sees no
crash nor `kill -9`; so was a shell in the engine's group, which Ctrl-C's or a closed terminal's signal to that group
ends with the engine; so was letting the shell go whenever the engine drops it, as an engine that ends by itself drops
the rounds it runs as it ends; and so was running a program with no shell, which would be the fault guarded against,
seen by nobody (principle nothing-passes-for-normal).

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

### decision the-agents-commands-are-given-no-credentials
The agents' program is given `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` set, so that it strips every credential from the
environment of the commands, hooks and servers it starts, keeping it for its own calls: the token of the account a
round runs under, and a credential `agents.env` names, reach the program and none of an agent's commands.
Why: Without it every command an agent runs would hold the token, and one that prints its environment would write it
into the round's stream, which is kept. It does not keep the token from an agent that asks the keychain for it with
`security`; that this is enough rests on judgment, not a trial.

### decision the-programs-directory-is-the-engines
The agents' program's directory of settings and sessions is the engine's, `profile/` in the data directory, made where
it is missing and given as `CLAUDE_CONFIG_DIR` to every command of the program's the engine runs; each try records it as
the directory it ran with, and `agents.config_dir` is refused as the engine starts, saying where it went (decision
a-moved-setting-is-refused-with-where-it-went). Every command is given, in the settings the engine passes it
(`--settings`), that the program's cleanup keeps a session a thousand years (`cleanupPeriodDays`).
Why: The sessions of every work live there, under every account (decision
a-line-goes-on-in-its-session-under-another-account), so the directory is the engine's to keep and no setting to point
elsewhere; with none named the program would use its default one, which an agent can write to. Its cleanup deletes a
session after thirty days by default, and a work can wait longer with its session to go on in. Given with every
command, the cleanup holds whatever the directory's own files say, where a file the engine wrote into the directory
would be one more thing an agent or a hand could change; that the program takes the key from `--settings` rests on its
documentation (any settings file, and `--settings`, may set it), not a trial.

### decision the-pages-server-is-off-the-loops-thread
The server of the pages and the interface runs on threads apart from the one the engine's loop takes things on, and
the engine ends when either of the two ends.
Why: The loop answers a read on its own thread, and a server waited on together with the loop on that thread accepted
no connection while it did: with 337 works, a stylesheet asked for during a read of the items waited 2.4 s. Waiting on
the two together on the loop's thread, the plain way to end the engine with either, was turned down for that. Why
they share a process at all is decision one-process-serves-everything.
