# A command that reaches the network or writes beside the code change runs as an engine tool

## What is done now

A command an agent needs that fetches outside material from the network, or writes files beside the change it makes to
the code, is run by the engine as a tool drawn on the flow, never by the agent in its own shell. The command is written
in the project's configuration and not chosen by the agent: the agent asks for the tool by name with the one input it
takes, the engine runs the configured command in the work's worktree, records each attempt in `tool_runs`, and gives
what became of it as the tool's result on the next round.

The first instance is the tool `dependency_sources`. The agent names a lock file of the project; the engine finds it
among the project's `[[dependency_sources]]` tables, checks with git that the directory the table names is ignored,
runs the table's command (split into words, run without a shell, in a process group of its own, with a time limit that
stops it and everything it started), and gives `fetched` with the directory, `failed` with the exit code, whether it was
stopped for time and the output, or `refused` with the reason when no command ran. A command that fails while the
network is down makes the cell wait for the network, as every tool does. The cells that research, design, implement,
review, test and advise are offered the tool. The engine knows no language and no lock file format; a project of any
language writes the command of its own. The types and the examples are in `crates/config/src/lib.rs`
(`DependencySources`), the tool's text is in `flow/flow.json`, and the README says how a project sets it up.

## Why

- Article 1: nothing rests on a feature of one agent's command line. A permission rule that matches one exact command
  line is such a feature; a tool the agent asks for in its return works under any agent command line.
- Article 7: a step that changes what is on disk is a cell or a row, on the chart the owner reads. A command in an
  agent's shell is on no chart.
- Article 4: each attempt is a row of `tool_runs`, so failures to fetch can be summed up and a refusal is as visible as a
  run.
- Article 2: every cell that judges a design or a change can read what a dependency does, and need not write it as
  inferred or put the question to the architect, as matter 177 did for a component's types.
- Article 3: the price is one more round for each request. Measured from the engine's records, a tool round takes 5.0 s
  at the median and 13.0 s at the 90th percentile, and the command's own time comes on top; the owner accepted it.
  Only a work that needs the source asks, and the package manager's own download cache is shared by every work.

## Options not chosen

- The agent runs the configured command in its own shell, which the engine lets through on the agent's command line and
  names in the work's message. It saves the round. It was not chosen because the exact-command permission is a feature
  of one agent's command line (Article 1), and the run is on no chart and in none of the engine's records (Articles 7
  and 4). Costing the engine a flow change and some code was no reason for it: a design is judged on itself (Article 6).
- The engine fetches the sources by itself on every round into a cache shared by the works. It is a step outside the
  flow (Article 7), done whether or not the round needs it.
- The agent fetches one package at a time. It is tied to a language, and nothing makes it fetch the version the lock file
  locks.
- The agent reads a package's repository on GitHub. The published package holds what the repository does not (the
  compiled `dist` and the types of `@primer/react` 38.41.0 are not in the repository at that version's tag).
- The agent reads the machine's own package cache. It depends on where a machine keeps it, and the engine's machine is
  not the agent's.

## Not covered

The lock commands of #266 have the tool `lock_update` (#280). The prefix permissions that let them run in the agent's
shell stay until #283 removes them, once the tool has run on a real machine.

What a command the engine runs can take from the files an agent writes in the worktree, and from the engine's own
environment, is not settled: #284.

## How it was decided

The owner's choice at decision point P1 of the design of item #208 was made by the assistant on the owner's behalf, under
the owner's authorization on the evening of 2026-11-04 (the owner's asks 58 and 60), and is to be confirmed by the
owner's own review of this change. A different choice made there replaces this file.

## Challenges considered and turned down

None yet.
