- The engine advances works in parallel, and the architect's and engineers' state machines run at once. Otherwise it is
  too slow.

- The global switch and a single work's switch go by whichever was pressed last: a work paused alone is not released by
  a global start and needs the owner's own start, and a single start's mark is cleared by a global pause. A work paused
  alone usually has a problem and is being held; a single start is an exception during a global pause, and pressing
  global pause again means all stop.

- The engine starts with every work paused, nothing running until the owner starts all or one: after an upgrade or a
  restart from a fault, the owner sees the state before works spend money again.

- The hold on a project's new works is kept across a restart, while Pause all is not, the engine starting with every
  work paused (backend/engine/scheduling.md, "The engine starts with every work paused"); neither sets or lifts the
  other. The hold is a standing decision about the project, Pause all a moment's.

- The owner may release any hold on a project's new works, the architect's as well as its own. An architect's hold the
  architect does not release would otherwise leave the project stopped, the owner's only way being to ask the architect,
  one round later.

- The engine can step: one round at a time, after which the work stops as if paused by the owner, and the next runs once
  this one looked fine. A hundred and one rounds in a row once missed a problem there from the first.
