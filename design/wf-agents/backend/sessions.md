- What every session of a line shares goes in the system prompt a new session of the line starts with, ahead of all that
  differs by session (wf-agents.md, "What an agent is sent is ordered by how widely it is the same").

- A waiting session's cache is kept warm by a ping 55 minutes after its last call, before the cache lets it go, for no
  more pings than the wait is worth at the model's prices, since a ping costs little, about $0.03 as measured, and
  resuming a session the cache let go is very costly.

- The architect's base is warmed by one ping before a fork is made, unless its last call is a ping under 55 minutes old,
  and is kept warm while the architect has a work running, since a fork reads the base from the cache only up to a call
  the cache still keeps. Measured, a fork that missed wrote some 73K of the base's 107K tokens again, $0.57 more against
  a ping's $0.03, and every fork after it did the same.
