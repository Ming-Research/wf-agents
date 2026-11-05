- An agent that must query the database or run a tool does so by returning, the engine running it, and going on from the
  last round, never in mid-round; such a return need not carry progress. To the model both are one tool call, with the
  same in its context and the same count, and querying mid-round saves about three seconds each.

- How to query is in the tool, and the database's whole structure is not put into an agent's context: at over 20K that
  is very costly.

- No tool opens a page in a browser or takes screenshots; searching and fetching the web are another matter. A project's
  page is tested with a browser in CI, and an agent needs none on its machine.

- An engineer can take generated files back from CI. Unable to compile locally and unable to get generated files right
  by hand, an item would stop to wait for someone.

- A command an agent needs that reaches the network or writes files beside its change to the code is run by the engine
  as a tool drawn on the flow, never in the agent's own shell: the project's configuration names the command, and the
  agent asks for the tool by name. Letting one exact command through is a feature of one agent's command line, and a
  command in an agent's shell is on no chart and in none of the engine's records. The price is one more round for each
  request, about five seconds at the median beside the command's own time. Rejected: the agent running the configured
  command itself; the engine fetching on every round whether needed or not; fetching one package at a time, which is
  tied to a language; and reading a package's repository or the machine's cache, which lack what is published or depend
  on the machine.
