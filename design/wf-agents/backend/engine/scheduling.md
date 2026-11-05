- The engine advances works in parallel, and the architect's and engineers' state machines run at once. Otherwise it is
  too slow.

- The global switch and a single work's switch go by whichever was pressed last: a work paused alone is not released by
  a global start and needs the owner's own start, and a single start's mark is cleared by a global pause. A work paused
  alone usually has a problem and is being held; a single start is an exception during a global pause, and pressing
  global pause again means all stop.

- The engine can step: one round at a time, after which the work stops as if paused by the owner, and the next runs once
  this one looked fine. A hundred and one rounds in a row once missed a problem there from the first.
