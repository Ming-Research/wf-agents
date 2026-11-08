- Failures from the network being down are treated alike: agents, the engine's own tools and git operations stop, the
  engine probes every half minute, and when it is back each goes on from its cell, with no asking of the architect and
  no pause. Retrying during an outage wrote the whole input into one session again and again, filling it; and with only
  the agent's round waiting, a tool step would go to ask an architect who was paused, each waiting on the other.

- A round that did not run for a reason such as a failed login asks the owner to write nothing: once the owner has
  handled it, by logging in again, the work goes on.

- Being stopped by the service's safety check is a fault of its own kind: the first retries in a new session, and a
  second pauses the work and tells the flow's keeper. It more likely comes from a session full of repeats than from the
  prompt's words, though this cannot be proved.

- Too many send-backs in a row stop the work and tell the flow's keeper. Otherwise a cell could keep being sent back,
  rerun and spending.

- Each return sent back is recorded on its round, in the same write that ends the round, and the work does not wait for
  its cause to be looked for: the cell runs again at once. Each cause is to be found once and mended (Article 4), and a
  look afterwards loses nothing, the problems being kept on the round, while a work that paused at every return sent
  back kept agents idle for it (Article 3). The record is sent to no one as a message, since every return sent back
  would then reach the flow's keeper; how many are sent back is watched as the system's other measures are
  (backend/engine/health.md, "Running safely is made the whole system's state control"; frontend/system.md, "The system
  page is all the metrics"). Recording only the return that pauses the work was rejected, since causes that never reach
  the limit would never be looked at; so was recording at the pause or at the next step, which a pause and a start
  between them can lose.
