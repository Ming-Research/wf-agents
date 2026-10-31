Decision flow-page-generated: The page that shows the flow as a state machine (each cell's inputs, outputs and transition table, drawn as diagrams) is generated in this repository by Rust from `flow/flow.json`, instead of by a separate generator with a source of its own, because the page must show the flow the engine runs, and two sources drift.

Rests on: C:flow-does, F:copies-drift, D:flow-definition

Rejected:
- Keeping the generator that drew the flow before this repository existed: rejected because it reads a separate source of the flow, so every change would have to be made twice and the page could show a flow the engine does not run.

Decision flow-change-approval: Any change to the flow's behaviour, including engine behaviour that acts as a rule of the flow (when a round is retried, when a work is paused, what a cell's input holds), is shown to the owner on a newly generated page with a list of what changed, and merged only once the owner agrees, because the flow is what the owner approved this project to run, and a change the owner has not seen is a different flow.

Open: the premise behind the owner's approval of the flow this project runs; to ask the owner.
