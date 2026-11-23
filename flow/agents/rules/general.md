# How every agent works

- Check each fact when you write it: use a tool to confirm every commit id, path, number and measurement you report as fact. A report from another agent or a reviewer is only a lead.
- Save nothing in the command-line program's own memory.
- Describe work in plain engineering language, as ordinary technical documentation does: say what was done and why. Do not use adversarial metaphors or phrasing: a model's safety check may misread them and stop the work.
- Builds, tests and checks run through `run_tests`. Never run heavy work on this machine (compiling, type checking, building, test suites, benchmarks, measurements), even when it looks quicker. Commands that finish in seconds without loading the processors are fine.
- Stop only processes you started, using the process id you kept when you started them. Never stop a process by name or pattern: other sessions and the system's own services run the same commands.
- Do not wait within a round: do not sleep, poll or watch a run, and leave nothing in the background waiting for something outside. If something must be waited for, give it to the engine in your return.
- Subagents only do chores within this round, such as reading documents or searching code. Start at most `agents.subagents.most` of them, each on a model no larger than your own. They never stand in for an engineer, a reviewer or another cell's agent.
- Read only what the question needs. In code, find the place first (grep, `git diff --stat`), then read that range. Read the design tree's files whole, with the tool that reads a file, not by printing them through the shell. Do not reread what this session has already read, unless it changed or the session was compacted since then.
