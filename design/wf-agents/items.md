- The two kinds of record are called items and observations, not commitments, since everything in the system says item
  (wf-agents.md, "A thing is named for what it is").

- Items have only dependencies between them, and together form a directed acyclic graph: an item owns nothing, a
  relation does not double as a category, and if A depends on B, A cannot finish before B. Relations that stood for both
  dependency and category ended in a tangle, and items owning sub-items beside plan rows that ordered them gave two
  relations to keep apart.

- The items are laid out as a tree along their dependencies: if A depends on B, B is below A, directly or under items
  below A. Reading down from an item then shows all it needs, and how far it has come is how far what is below it has
  come. A tree drawn by anything else beside the dependencies would be a second relation to keep in step with them.

- An item stands in one place, under the item whose dependency on it is its main one; below every other item that
  depends on it, it is a line pointing to that place. Of the dependencies on an item, exactly one is main at all
  times: the most fitting, the first made until another is chosen, and the earliest made of those left when it goes;
  never that of an item that also depends on it through another, which would set it beside an item that needs it.
  Shown in full in several places, an item would be counted, read and answered more than once, and each thing has one
  place. The place is a mark on a dependency rather than on the item, so that an item never stands under one that does
  not need it.

- Items side by side under one item do not depend on one another and can be started at the same time. Where one must
  be done before another can be, that is a dependency, which puts the one below the other; which of two free items
  starts first when slots are few is only the order the architect's plan gives them.

- A goal other than a root has two items or more under it; with one, that item takes its place. A goal is done exactly
  when what is under it is, so a goal over a single item adds a level and says nothing, as a group of one is no group. A
  root, a project's direction, may stand alone until its items come.

- An item that is done never depends on one that is not: a dependency added to it is refused, and what is left goes to
  a new item. A dependency is what must be done before the item can be, so a done item standing over an unfinished one
  would say both. A goal, done only while all it depends on is, takes the dependency and is open again. An item whose
  work has passed the merge gate counts as done here: the gate is the one place a work waits for what it depends on, so
  a dependency added after it would be waited for by nothing, and the item would end done over it all the same.

- An item with more than ten dependencies side by side has them gathered under items between, each delivering one part
  of what it needs; the items gathered keep their own works. A long row under one item is the flat backlog the tree is
  there to replace: it cannot be taken in at a glance, nor does it show how far each part has come.

- An item has three states: open, done and cancelled. It is done once a work of it reached the end of its graph, a goal
  once all it depends on is done, and open otherwise, also when its last work ended before its end. What its work is
  doing, what the architect's plan does with it and what it waits for are facts of the work, the plan and the
  dependencies, never the item's state. One state that mixed them read one item five ways, its states contradicted one
  another, and an item whose work stopped read as stopped though nothing was delivered or given up.

- A work has a state of its own: where it stands in the flow, and what it is doing there: running; waiting, and for
  what, such as the owner's answer, another item, a slot or a retry; paused, and by whom; or ended, and whether it
  reached its end. An item has works one after another, at most one going on at a time, and the item's state does not
  repeat theirs.

- An observation records only its title, what it is, why it is worth doing and what happens if not, its tags, source,
  times reported and references, with no condition and no event. A condition is one sentence no mechanism checks, and
  events are too few in kind to use.
