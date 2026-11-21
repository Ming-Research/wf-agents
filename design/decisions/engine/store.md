# The store

Where the records of the works are kept, and how the store's tables change.

### decision tables-only-added-to
The tables the deciding code is given are only added to, each row at its place in the order of what happened; there is no general store of events beside them.
- replaced: Tables whose rows each transaction set to the current state, a record of the changes beside them, stood on main; the owner changed it, because the deciding code is given a work's records as they were at a moment, which tables only added to give with no copy kept.
- rejected: an agent proposed a log of every event only added to, with snapshots now and then and the current state in memory; the owner rejected it, because SQL would go round it for nothing, one transaction giving the state, its acts and its history together.

### decision migrations-are-named-by-their-minute
A migration of the store is a file named by the minute it was written and what it does, and a database made with an earlier version of the schema is brought on by each in turn as it is opened.
- replaced: Migrations numbered one after another stood on main; the owner changed it, because two works took the same number, and 17 of 51 merges went back for conflicts.
