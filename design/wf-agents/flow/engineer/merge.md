- A work that changes the flow waits for the owner's answer before merging. The owner is shown the flow page drawn from
  the branch's flow beside main's, and if the flow changes again it is drawn and asked again. Showing it first is
  enforced by the flow and rests on no one's memory. Which paths wait is set in the project's configuration, a mechanism
  open to any project. A rule in the project's guide alone, kept by the engineer and the review, was rejected: when both
  miss it, the change merges.

- The compiled flow is stored in the database with the version in force then, and the page draws from there, so it
  depends neither on git nor on today's compiler; each time costs one more stored flow. Compiling from a commit when
  opened was rejected: the commit may be cleaned up once the branch is merged and deleted, and a later compiler's format
  may not compile an old flow.

- The work that merged a change checks, once the change takes effect, what only real runs can show: a condition that no
  test, CI check or measurement on the branch can show says what carries the change, which tool's real run checks it,
  what holds, and what evidence it waits for. It waits holding no slot and keeping no session warm, and a condition that
  does not hold goes at once to the architect. Two limits in the project's settings, on waiting for the change to take
  effect and on looks, end the wait, with the architect told what stays unverified, since a merged work can be neither
  cancelled nor started over. A work of its own to check it was rejected, as a whole graph for a few conditions; so was
  the architect checking it, whose graph has no cell that waits and whose rounds cost more; and so was SQL in the
  condition for the engine to wait on, which cannot be tried when written and, once wrong, waits for ever.
