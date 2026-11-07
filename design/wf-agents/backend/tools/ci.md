- Test results are filled in by the engine from the JUnit artifacts CI uploads, with the project's configuration naming
  the artifact and the test name's separators, not reported by the agent; other projects can join with little adapting.
  An agent misreading results runs more, and when one unit does not compile the other tests have no results.

- Where a round runs both CI and the comparison on the code before the change, they run together. The comparison needs
  CI only to have pushed, not its result, which saves about three minutes a round, as measured.

- Where a project's CI takes no test name, the comparison still waits for the whole fast CI rather than refusing.
  Otherwise such a project's adversarial tests have no evidence of red before and green after, and the system must not
  serve wf-agents alone.
