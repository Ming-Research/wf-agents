- What is sent each round is kept tight: apart from what a line's cells share (wf-agents.md, "What an agent is sent is
  ordered by how widely it is the same"), nothing long, each place reviewed, and what means nothing removed. Wordiness
  wastes tokens.

- A message opens with the round's address, and what is given by session carries an address of a fixed form, with what
  the session has referred to by that address; the working directory, branch, task and review table are of this kind.
  What is referred to need not be something the engine gave and may be what the agent returned, and with no fixed form
  of address it could not be referred to.

- Before a message goes out, it is checked that what the session has was not sent whole again; if so the work stops and
  the flow's keeper is told. Sending again is a waste and a serious bug (flow/inputs.md, "A cell's inputs are of two
  kinds"). This took the place of a threshold of fixed size.

- The rules agents are given say how to read things, to reduce tokens. A small matter's token use looked too high.

- The form of a return is shared by all projects and says only in the owner's language, and the engine fills in the
  language on sending the form. The form is compiled from one flow, so it cannot name a language.

- Each line has one combined form of return, given to the agent's command line and shared by all the line's cells, each
  using its own branch; a planning session and those forked from it share one, and the whole form goes only in the
  planning session, once. The form is part of what the line's cells share (wf-agents.md, "What an agent is sent is
  ordered by how widely it is the same"): with one for each cell, changing cell in a session would lose the cache; and a
  fork continues the planning session and is no other line.

- An image for an agent goes in as part of the message, not as a path in the prompt for it to read. That needs no
  temporary files to store, no read permission to give, and no assuring that each kind of agent can read it.
