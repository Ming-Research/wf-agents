- wf-agents is for agents and the owner working together to try, explore and push a piece of software forward at scale.
  It is a tool for two, the owner and the assistant: they discuss the design freely and watch the work, and the system
  carries forward what they decide. One chat cannot push several tasks at once. A system that both ran the work and
  talked with the owner in place of that chat was tried: the side that talks became as busy as the architect, with no
  time left to discuss.

- What is good is judged by the owner's taste and use, with no user research or study of design guidelines, since
  wf-agents serves the owner. Only a product for the general public needs research and guidelines to reason from.

- A problem found is turned into a patch in the system by a means that converges, so that it does not happen again.
  Someone typing it into a chat each time does not scale, and a new cell on the graph for each patch does not converge
  either.

- Managing the project well is one of wf-agents' aims: using what AI can do to remove the bottleneck of traditional
  project management, and raising how much development runs at once by an order of magnitude. Once the flow that writes
  code runs, project management is where the work backs up. Periodic grooming of a backlog was not copied: even every
  two weeks it is tiring and inefficient.

- wf-agents is only the harness: the engine, the flow, the database's structure and the command line. A project's
  configuration, the rules it gives its agents and its data live in the owner's private data directory, split by project
  with the shared parts together; a project's design tree lives in the project's own repository. The harness must say
  nothing about any project, and the owner's projects share most of their settings, which are best kept together.
  Keeping them in this repository would put a project's content in the harness; keeping them in each project's
  repository would copy the shared settings many times.

- A project's constitution sits at the root of its repository, its configuration names the path, and every agent that
  judges is given it: it holds the values every agent judges by.

- What an agent is sent is ordered by how widely it is the same, since the prompt cache knows only a request's opening
  bytes: the first to write them pays, and later requests that open alike read them. First what every cell of every line
  shares, then what every cell of its line shares, its rules and its forms, then what is the same for every work of the
  cell, its task and the list of its inputs, then what is the same within a work, then what changes each round. Every
  cell then opens alike with what all cells share, the constitution first and the project's AGENTS.md after it, and any
  input all cells come to share goes there too; a cell that judges nothing may leave that opening out. Whether the cache
  holds is measured after each change of the order, from the cache read and written that real rounds record: an order
  meant to be cached was found not to be for a detail no one had seen, and a part placed after one that changes is
  written again by every work, which cost the first research a fifth of its price.

- The engine follows the compiled flow mechanically: it takes the row a return selects and runs the named actions and
  checks, and does nothing that needs judgment. Judgment belongs to the agents and the owner, and tools only execute and
  never choose the route. Judgment needs someone who can give reasons and be reviewed, and each step of the engine must
  be predictable from the flow.

- A thing is named for what it is, and has one name everywhere. A name for what it is needs no explaining; a name of its
  own, or a second name beside the first, is one more thing each reader must learn and tell apart.

- The part of the program that carries the state machine is called the engine (wf-agents.md, "A thing is named for what
  it is"). Calling it wf-agents would not tell it from the compiler and the session manager, which are in the program
  too.

- "Board" is no name: what means the data is called the database, and what means what the owner looks at, the owner's
  page (wf-agents.md, "A thing is named for what it is").

- Nothing is designed for the machine's lid closing or its sleeping: the machine that runs the engine stays on.

- A technology is chosen as mature, mainstream and fit for the project's scale, with no item-by-item comparison of its
  peers. Nobody would really make that comparison, and SQLite and React were not chosen that way.
