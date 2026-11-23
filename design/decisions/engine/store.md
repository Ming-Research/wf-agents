# The store

Where the records of the works are kept, and how the store's tables change.

### decision tables-only-added-to
Rows are only added to the tables the deciding code is given, each row in its place in the order of what happened; there is no general store of events beside them.
- replaced: Tables whose rows each transaction set to the current state, with a record of the changes beside them, was on main; the owner changed it, because the deciding code is given a work's records as they were at one moment, and tables that are only added to give that with no copy kept.
- rejected: an agent proposed a log of every event that is only added to, with snapshots now and then and the current state in memory; the owner rejected it, because the SQL would take a detour for nothing, when one transaction gives the state, its acts and its history together.

### decision wrong-costs-are-rewritten-once-by-a-command
`wf-agents repair-costs` rewrites the recorded cost of rounds and forks once, in place, from their kept streams, after a copy of the database: the one exception to decision tables-only-added-to.
- rejected: an agent proposed a table of corrections that the budget and the pages read before the recorded cost, leaving the rows only added to; the owner rejected it, because every place that reads a cost would carry one more layer for good, and the wrong numbers would stay in the database.
- rejected: an agent proposed changing only how a cost is recorded from now on and leaving what was recorded; the owner rejected it, because a cost recorded wrongly, such as one round's money charged to the next, would stay in the budget and on the page, and the cost was asked to be put right.

### decision migrations-are-named-by-their-minute
A migration of the store is a file named by the minute it was written and what it does; a database made with an earlier schema is brought up to date by each migration in turn when it is opened.
- replaced: Migrations numbered one after another was on main; the owner changed it, because two works took the same number, and 17 of 51 merges went back for conflicts.
