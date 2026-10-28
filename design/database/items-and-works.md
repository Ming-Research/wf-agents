Decision: Items own no other item: the only relation between items is dependency, and the items form a directed acyclic graph, checked for cycles on every write, instead of parent and child items, because an item that depends on others simply cannot be finished before them, and ownership adds nothing that dependency does not already say.

Decision: An item and a work are kept apart: the item (what to do) belongs to the architect and the owner, the work (one run of the flow on it) belongs to the engine, and an item can be stopped and started again as a new work, because what is wanted outlives any one attempt at it.

Decision: Works start first from the dispatch queue the owner orders by hand, then in the architect's order: each item by its rank among its project's items, lower first, an item placed nowhere yet, as one just made is, after those placed, in the order made; an item not cancelled, held or done is in that order, instead of only the owner's queue, because the architect, who sees the whole, decides what starts next, and holds what should not start yet; provisional: the owner's queue stays until the architect's order alone decides what starts.

Decision: The architect's order is a rank on each item, an integer within its project, which the architect's changes the owner approved set by moving an item first, last, or just before or after another item of its project, every item of the project ranked afresh, instead of the plan, segments the root items hung on with gates on what each might touch and budgets it might spend, because a change to a specification, a public interface or a data format is a design decision the owner confirms on the work's design card, and whether a work drifts from its direction is the architect's judgment, not a limit the engine measures.

Decision: The architect's matters are a table of their own, which records which side reports, questions and design cards the architect took and how it replied, instead of views over the rounds, because a matter has a state the rounds do not hold: whether it is due, held for the owner, replied to or handled.

Decision: Known red checks are a table, instead of static configuration, because the architect adds and removes them while works run.

Decision: Side reports are a view over the returns, and each is queued once as a matter of the architect's, which records that it was taken, instead of a table of side reports, because the report itself never changes, and what the architect does with it is the matter's.

Rejected:
- Parent items owning the child items split from them: rejected because a split is fully described by the parent depending on its children, and ownership would forbid an item serving two dependents.
- The plan (segments under the policy's stages that the root items hung on, gates on which segments might touch a specification, a public interface or a data format, and budgets shared out among them, checked before each merge): rejected because the design card already puts each such change to the owner, and a mechanical limit cannot tell drift in direction from a change the work needs.
