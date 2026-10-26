Decision: `flow/flow.json` is the only definition of the flow, written in English, and it says nothing about any project: every project-specific value is a slot the project's configuration fills, because the same flow must run any project, wf-agents included.

Decision: The flow is compiled by the Rust crate `flow`, which the engine and the command line both use: it expands the shared rows, the gates and the asking block, builds each cell's return schema for each line, and runs every check on the flow (formats and names, every state the engine can be in is reached, each possible return takes exactly one row, every recorded value is read, every loop has a counter, every input has a source, every rule is used, the samples match); the expanded flow and its hash are stored as a flow version, because a flow that fails a check must never reach a running work.

Decision: A work is pinned to the flow version it started with and runs on it to the end; a new version applies only to works started after it, because changing the flow under a running work could leave it in a cell or a state the new flow no longer has.

Rejected:
- Project names, branches or rules written into the flow: rejected because the flow would then work for one project only.
