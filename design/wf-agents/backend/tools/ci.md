- A work's branch runs a fast CI by hand that compiles and runs only the tests and checks named, in minutes, and can
  never let a head merge; the full CI runs once, at the merge, on a head it has not run on, and what it finds red goes
  back to the work, which fixes it with the fast CI. This is how the flow solves running everything for every work
  (flow/engineer.md, "Running everything for every work"), as Article 3 asks. A project whose CI cannot be run by hand
  has a push run its full CI, and the engine waits for it.

- CI run by hand on a work's branch runs with main's CI files, on a temporary branch that holds the work's head with
  them, since GitHub runs a workflow by the file the branch holds, and refuses to on a branch made before main's file
  took the request.

- The engine watches each project's main: when the full CI of main's latest commit is red, it stops the merge queue and
  runs the red checks again, and only a red that stays goes to the architect, as which merge made it red and whether to
  fix or revert it. Merging on a red main, one could not tell which merge made it red, and a red that passes when run
  again needs no architect.

- Test results are filled in by the engine from the JUnit artifacts CI uploads, with the project's configuration naming
  the artifact and the test name's separators, not reported by the agent; other projects can join with little adapting.
  An agent misreading results runs more, and when one unit does not compile the other tests have no results.

- Where a round runs both CI and the comparison on the code before the change, they run together. The comparison needs
  CI only to have pushed, not its result, which saves about three minutes a round, as measured.

- Where a project's CI takes no test name, the comparison still waits for the whole fast CI rather than refusing.
  Otherwise such a project's adversarial tests have no evidence of red before and green after, and the system must not
  serve wf-agents alone.
