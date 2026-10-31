Decision data-directory: The data directory, named by `--data` or the `WF_AGENTS_DATA` environment variable and never written in the code, holds data only: the configuration, the database, and the files the database refers to by path (logs of red checks and tool outputs too long to keep inline), because these cannot be rebuilt, belong to the owner and must be kept and backed up together.

Rests on: C:independent, C:design-on-merit, D:harness-only

Rejected:
- A separate run directory for logs, tool outputs, the engine's log and the stop file: rejected because the database refers to the logs and tool outputs by path, so they cannot be thrown away and splitting them from the database only makes them easier to lose.

Decision binary-apart: The program is not in the data directory: the `wf-agents` binary is installed on its own, separately from any data, because the program is public and replaceable while the data is private and lasting, as a public program repository sits beside a private data repository.

Rests on: D:harness-only, D:data-directory

Rejected:
- The binary kept in the data directory: rejected because it mixes the program with the data it works on, so replacing the program touches the data directory and the data directory is no longer data only.

Decision worktrees-in-data: The works' worktrees are in the data directory, under `worktrees/`, which the data directory's git leaves out, instead of a directory the installation configuration names, because they are this installation's working copies, kept with the data they belong to, while the directories beside the projects' own checkouts hold the owner's working copies, not the engine's; the data directory is in git, and ignoring them keeps it data.

Rests on: F:owner-working-copies, D:data-directory, D:config-committed

Rejected:
- The works' worktrees beside the projects' other checkouts, in a directory the installation configuration names: rejected because those are the owner's own working copies of the projects, and a setting for where the engine's worktrees go names nothing the data directory does not already hold.
