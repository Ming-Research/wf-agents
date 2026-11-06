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

- The engine follows the compiled flow mechanically: it takes the row a return selects and runs the named actions and
  checks, and does nothing that needs judgment. Judgment belongs to the agents and the owner, and tools only execute and
  never choose the route. Judgment needs someone who can give reasons and be reviewed, and each step of the engine must
  be predictable from the flow.

- A project's constitution sits at the root of its repository, its configuration names the path, and every agent is
  given it: it holds the values every agent judges by.

- Items have only dependencies between them, and together form a directed acyclic graph: an item owns nothing, a
  relation does not double as a category, and if A depends on B, A cannot finish before B. Relations that stood for both
  dependency and category ended in a tangle, and items owning sub-items beside plan rows that ordered them gave two
  relations to keep apart.

- The items form a tree by their dependencies: if A depends on B, B stands below A, directly or under items that stand
  below A. Reading down from an item then shows all it needs, and how far it has come is how far what stands below it
  has come. A tree drawn by anything else beside the dependencies would be a second relation to keep in step with them.

- An item stands in one place, the most fitting of the items that depend on it, and the dependencies of the others
  point to it there. Shown in several places, it would be counted, read and answered more than once, and each thing
  has one place.

- Items side by side under one item do not depend on one another and can be started at the same time; an order between
  two is a dependency, which puts one below the other. What can start now is then what has nothing left undone below
  it, read straight off the tree.

- Nothing is designed for the machine's lid closing or its sleeping: the machine that runs the engine stays on.

- A technology is chosen as mature, mainstream and fit for the project's scale, with no item-by-item comparison of its
  peers. Nobody would really make that comparison, and SQLite and React were not chosen that way.

- The part of the program that carries the state machine is called the engine. The name says what it does. Calling it
  wf-agents would not tell it from the compiler and the session manager, which are in the program too, and a name of its
  own is one more thing each reader must first learn.

- "Board" is no name of its own: what means the data is called the database, and what means what the owner looks at is
  called the owner's page. Named for what it is, it needs no explaining; with a name of its own, each reader must first
  work out which of the two it means.
