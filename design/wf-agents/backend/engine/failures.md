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
