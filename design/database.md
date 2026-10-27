Decision: wf-agents keeps its data in SQLite, where the plain tables are the true data, and one write is one transaction that changes the current state, records the change and queues the actions to take (the outbox); the outbox is worked off after the commit, each action deleted once done, and after a crash the remaining actions are done first, because a crash then neither loses an effect nor performs it twice.

Decision: `db/schema.sql` is the only definition of the tables and views: each table and field says what it is, what problem it solves and who reads it; the program builds the database from it, the table descriptions agents are given are read from it, and the database is changed only by changing it, with the migration in the same pull request, instead of tables defined across the code, because every later change needs one source to go by.

Decision: The database holds only wf-agents' own things; the configuration files, the flow, the projects' repositories (code, design tree, research notes), GitHub (branches, pull requests, checks), the worktrees and the command lines' session records are only referred to, instead of copied, because a copy would have to be kept in step with its source.

Decision: A return is stored whole as JSON, and only what is filtered or joined on is split into columns (the work, the cell, the transition taken, the times), instead of a column per returned field, because the returns' shapes come from the flow and change with it, while the columns stay the same.

Decision: What can be computed is not stored: counters, total time, stages, anomaly summaries, what the engine accumulates and free slots are views, instead of stored fields, because a stored summary can disagree with the history it summarizes.

Rejected:
- A column per returned field: rejected because every change to a cell's return would need a migration, while the JSON keeps the whole return as it was handed in.
