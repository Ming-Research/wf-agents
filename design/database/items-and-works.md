Decision: Items own no other item: the only relation between items is dependency, and the items form a directed acyclic graph, checked for cycles on every write, instead of parent and child items, because an item that depends on others simply cannot be finished before them, and ownership adds nothing that dependency does not already say.

Decision: An item and a work are kept apart: the item (what to do) belongs to the architect and the owner, the work (one run of the flow on it) belongs to the engine, and an item can be stopped and started again as a new work, because what is wanted outlives any one attempt at it.

Decision: The dispatch queue is a table the owner orders by hand, instead of an order derived from a plan; provisional: there is no plan yet to derive it from, and the table is dropped once the plan gives the order.

Decision: Known red checks are a table, instead of static configuration, because the architect adds and removes them while works run.

Decision: Side reports are a view over the returns, instead of a table, because while no flow takes them they have no state of their own to keep; a table that records which reports were taken is added when the architect's flow takes them.

Rejected:
- Parent items owning the child items split from them: rejected because a split is fully described by the parent depending on its children, and ownership would forbid an item serving two dependents.
