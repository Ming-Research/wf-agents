# The design tree

1. The design tree is `design/`: the constitution, `principles/`, and files of decisions by topic. An agent the engine runs is given the constitution and `principles/` whole; any other agent reads them before it designs or changes code.
2. Read it for what was decided and why, not as orders: where the right solution needs a decision changed, propose the change.
3. Its files of decisions hold only what the code on main does: an entry enters with the change that makes it true, and a change that makes one untrue rewrites or deletes it in the same work. A design not yet true of main waits in `pending-design.md`. The principles are the owner's and may come before the code.
4. Only the owner changes the constitution and `principles/`: where one needs changing, or the right solution breaks one, the owner decides, an agent the engine runs asking by a question with `stop` and any other telling whoever gave it its task.
5. Before changing code, read the design decisions and documents that govern it.
6. What the tree records, as the owner settled it for every project; it is brought here unchanged:
   - Only key technical decisions: those that shape later implementation or the overall architecture. Details stay out. Keep the granularity: too fine and decisions drown in detail; too coarse and important choices go unguarded.
   - A decision is a material choice between viable alternatives, even when the choice seems obvious: one that changes accepted behavior or persisted state, a safety or trust condition, a shared interface or representation, a significant performance commitment or a standing project rule. Task size and file count do not decide it. A step with only one viable way needs no record. When the tree grows too fine or too thin, the owner adjusts this threshold.
7. Keeping the tree lean. Pass every tree change through three filters:
   1. Decision, not description: remove a node that only describes.
   2. Grounds, not code: a node records why a choice was made and what it beat. An interface or implementation with no alternative behind it gets no node.
   3. Normalize upward: state a shared rule once, at the common ancestor.
