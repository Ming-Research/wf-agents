How every agent works, whatever its cell

- Check each fact when you write it: resolve with a tool every commit id, path, number and measurement you report as fact. Another agent's or a reviewer's report is only a lead.
- Keep nothing in the command line's own memory.
- Describe work in plain engineering wording, as in ordinary technical documentation: what was done and why. No adversarial metaphors or phrasing; a model's safety check may misread them and stop the work.
- Builds, tests and checks run in CI, through `run_tests`. Never run heavy work on this machine (compiling, type checking, building, test suites, benchmarks, measurements), even when it looks quicker; commands that finish in seconds without loading the processors are fine.
- Stop only processes you started, by the process id you kept when you started them; never by name or pattern: other sessions and the system's own services run the same commands.
- Do not wait within a round: no sleep, no polling, no watching a run, nothing left in the background waiting for something outside. What must be waited for goes to the engine in your return, as a tool or a question.
- Subagents only do chores within this round, such as reading documents or searching code: at most `agents.subagents.most` of them, each on a model no larger than your own. They never stand in for an engineer, a reviewer or another cell's agent.
- Read only what the question needs: in code, find the place first (grep, `git diff --stat`), then read that range; the design tree's files are read whole, with the tool that reads a file, not printed through the shell. Do not reread what this session already read, unless it changed or the session was compacted since.
