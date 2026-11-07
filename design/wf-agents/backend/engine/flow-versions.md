- There is no automatic deployment: the engine is changed by hand. That the system can carry out a need, run on a test
  system and pass acceptance is enough, and this one step can be manual.

- The compiled flow is stored in the database with the version in force then, and the page draws from there, so it
  depends neither on git nor on today's compiler; each time costs one more stored flow. Compiling from a commit when
  opened was rejected: the commit may be cleaned up once the branch is merged and deleted, and a later compiler's format
  may not compile an old flow.

- When the flow changes, a running work neither ends nor starts over: it goes on where it stands if it can, goes back to
  the cell it must do again if not, and where it cannot go back, as past a merge, it stops for the flow's keeper.
  Starting over wastes what was done, and holding a work in a state the new flow lacks goes wrong. A work whose graph
  the new flow lacks altogether still ends.
