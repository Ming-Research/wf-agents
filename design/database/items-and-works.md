Decision: Items own no other item: the only relation between items is dependency, and the items form a directed acyclic graph, checked for cycles on every write, instead of parent and child items, because an item that depends on others simply cannot be finished before them, and ownership adds nothing that dependency does not already say.

Decision: An item and a work are kept apart: the item (what to do) belongs to the architect and the owner, the work (one run of the flow on it) belongs to the engine, and an item can be stopped and started again as a new work, because what is wanted outlives any one attempt at it.

Decision: Works start first from the dispatch queue the owner orders by hand, then from the plan: the items that hang on its segments not deferred, whose earlier segments are done, by the segments' order, an item others depend on before them, instead of only the owner's queue; provisional: the owner's queue stays while the plan has few segments, and goes once the plan gives the whole order.

Decision: The plan is three tables written only by the architect's changes the owner approved: its segments (each a piece of delivery under a stage of the policy, the top of a project's items), its gates (which segments may touch the specification, a public interface or a data format) and its budgets (a number measured with a line shared out among the segments); a root item hangs on a segment by a column of the item, and an item others depend on goes with them, instead of segments as items, because an item that no engineer works on would carry fields that mean nothing for it, and with dependencies the only relation between items, the segment at the root is the one place an item's place in the plan is written.

Decision: The architect's matters are a table of their own, which records which side reports, questions and design cards the architect took and how it replied, instead of views over the rounds, because a matter has a state the rounds do not hold: whether it is due, held for the owner, replied to or handled.

Decision: Known red checks are a table, instead of static configuration, because the architect adds and removes them while works run.

Decision: Side reports are a view over the returns, and each is queued once as a matter of the architect's, which records that it was taken, instead of a table of side reports, because the report itself never changes, and what the architect does with it is the matter's.

Rejected:
- Parent items owning the child items split from them: rejected because a split is fully described by the parent depending on its children, and ownership would forbid an item serving two dependents.
