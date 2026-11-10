# The design tree

1. The design tree is `design/`: the constitution, `principles/`, and files of decisions by topic. An agent the engine runs is given the constitution and `principles/` whole; any other agent reads them before it designs or changes code.
2. Read it for what was decided and why, not as orders: where the right solution needs a decision changed, propose the change.
3. Its files of decisions hold only what the code on main does: an entry enters with the change that makes it true, and a change that makes one untrue rewrites or deletes it in the same work. A design not yet true of main waits in `pending-design.md`. The principles are the owner's and may come before the code.
4. Only the owner changes the constitution and `principles/`: where one needs changing, or the right solution breaks one, the owner decides, an agent the engine runs asking by a question with `stop` and any other telling whoever gave it its task.
5. Before changing code, read the design decisions and documents that govern it.
