Decision reasoning-system-copy: The installation keeps one copy of the reasoning system, a clone its `install.toml` names (`[reasoning] system`, by default `reasoning-system` in the data directory), which every project that keeps records is kept by: the engine runs its check on a project's records and reads its rules from there, and a project's repository holds only its records; a project keeps records when its configuration names them (`records`: the directory that holds them, and whether they are in English only), instead of each project holding the reasoning system as a submodule, because a project needs of the system only its check and its rules, the same for every project, and a submodule is a copy of them in every repository, which the engine's worktrees would not hold either.

Rests on: C:independent, F:copies-drift, F:worktree-no-submodule, D:harness-only

Rejected:
- The reasoning system as a submodule of each project's repository, its check and rules read from the work's worktree: rejected because every project then keeps its own copy of the same files, which drift, and a worktree the engine adds holds no submodule until it is initialised in it.

Decision records-check: A project's records are checked by the reasoning system's check (`check.py`), in the project's CI and at cell `D6` of every work whose branch changed them, which runs the installation's copy on the records in the work's worktree with the fork point from main as `--base` and with `--ready`, so that every record changed is named in a new entry of their log; its red goes back to the design (`D4`) by a row, with what it printed, instead of a check that sends the design's return back, because a red check of the records being drafted is ordinary drafting, which a row shows on the chart, while returns sent back three in a row pause the work.

Rests on: C:flow-does, D:failure-rows, D:reasoning-system-copy

Rejected:
- wf-agents' own check of the design tree (`wf-agents tree-check`), which counted the nodes and held a node changed since the fork point to the design tree's node format: rejected because the records moved under the reasoning system's format, which its check holds; one format, one check.
- The records check as a check of the design's return, sending it back when red: rejected because a red draft then counts toward the pause after three returns sent back in a row, and the chart shows no way back to the design.

Decision reasoning-rules-by-role: The cells that work on a project's records are given the reasoning system's rules by role, read from the installation's copy, each an input given only when the project keeps records, as the cell's panel says: reading (`reading.md`) to every cell that reads them (`D1`, `D3`, `D4`, `D7`, `T5`); writing and the cards extension (`writing.md`, `extension-cards.md`) to the design that writes them (`D4`) and to `D7`, which checks against them; the work extension (`extension-work.md`) to `D1`, `D3` and `T5`; the design coherence review (`dcr.md`) to `D7`; and the command that runs the check to the cells that run it themselves; instead of the flow's own rules of how the design tree is written and reviewed, because the rules records are kept by are the reasoning system's, kept with its check, and a copy of them in the flow drifts, while a project that keeps no records has none of them to work by.

Rests on: C:information, F:copies-drift, F:unmeetable-invites-misfit, D:reasoning-system-copy

Rejected:
- The flow's rules of design tree writing and design tree review, given to the cells that design, implement, fix and review: rejected because they were the flow's copy of a format and a review the reasoning system now holds, which drifts from it.
