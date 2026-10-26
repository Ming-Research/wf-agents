Decision: The data directory, named by `--data` or the `WF_AGENTS_DATA` environment variable and never written in the code, holds data only: the configuration, the database, and the files the database refers to by path (logs of red checks and tool outputs too long to keep inline), because these cannot be rebuilt, belong to the owner and must be kept and backed up together.

Decision: The program is not in the data directory: the `wf-agents` binary is installed on its own, separately from any data, because the program is public and replaceable while the data is private and lasting, as a public program repository sits beside a private data repository.

Decision: Worktrees are not in the data directory: each work's worktree goes in the worktrees directory the installation configuration names, beside the other checkouts of the projects, because a worktree is a working copy of a project's repository whose committed content lives on in its branch, not a record of the owner's.

Rejected:
- A separate run directory for logs, tool outputs, the engine's log and the stop file: rejected because the database refers to the logs and tool outputs by path, so they cannot be thrown away and splitting them from the database only makes them easier to lose.
- The binary kept in the data directory: rejected because it mixes the program with the data it works on, so replacing the program touches the data directory and the data directory is no longer data only.
