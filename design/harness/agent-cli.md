Decision: An agent's round runs as one call of an agent command line in its print mode (`claude -p` and the like), resumed in the same session for the next round, instead of an agent software development kit, because the owner can then switch accounts in a terminal and switch to another vendor's command line, and which command line runs and how it starts is configuration.

Decision: Each line of a work (the engineer, the review, the adversarial tester, the advisor) has one session, which the command line compacts itself when it grows, instead of starting a new session of the same line at a size limit as the flow describes; provisional: compaction is good enough to start with, and starting a new session is reopened when compaction loses what a line needs.

Decision: The command line is given one structured-output schema that is the same for every cell (the return's top-level fields with their types), while the cell's own schema goes in the prompt and the engine checks the return against it and says where it does not fit, instead of the cell's schema as the command line's structured-output schema, because the command line places that schema with the tool definitions, so a schema that changes from cell to cell invalidates the prompt cache at every change of cell.

Decision: An agent asks for the GitHub and database query tools by handing in `tools`, which the engine runs in its tool cell and gives back in the next round, instead of tools the agent calls in the middle of a round, because to the model either is one tool call, and going through the tool cell records every run and checks it against the list of allowed commands.

Decision: The command line reads only the settings of the agents' own configuration directory (`--setting-sources user` for `claude`), so it reads no project instructions or project settings by itself, such as the worktree's AGENTS.md, instead of reading them as a session started in a project directory does, because the engine gives the project's guide as a material once in a session, and a guide the command line reads by itself comes twice and comes back in every session whether the session needs it or not.

Rejected:
- Starting the command line bare (`--bare`), which also reads no project instructions: rejected because it takes only an API key, not the account the owner logged in with.
- A structured-output schema of a bare object: rejected because the model then handed in the nested objects as JSON strings, seen on three returns in a row.
- Tools served to the agent in the middle of a round: rejected because it saves the model nothing, and the costs of handing back (fields a return must carry, a restart of the command line) belong to how a return is handed in and can be cut there.
