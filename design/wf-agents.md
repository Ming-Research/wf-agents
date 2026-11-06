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

- The items are laid out as a tree along their dependencies: if A depends on B, B is below A, directly or under items
  below A. Reading down from an item then shows all it needs, and how far it has come is how far what is below it has
  come. A tree drawn by anything else beside the dependencies would be a second relation to keep in step with them.

- An item stands in one place, under the item whose dependency on it is its main one; below every other item that
  depends on it, it is a line pointing to that place. Of the dependencies on an item, exactly one is main at all
  times: the most fitting, the first made until another is chosen, and the earliest made of those left when it goes.
  Shown in full in several places, an item would be counted, read and answered more than once, and each thing has one
  place. The place is a mark on a dependency rather than on the item, so that an item never stands under one that does
  not need it.

- Items side by side under one item do not depend on one another and can be started at the same time. Where one must
  be done before another can be, that is a dependency, which puts the one below the other; which of two free items
  starts first when slots are few is only their order in the queue.

- Nothing is designed for the machine's lid closing or its sleeping: the machine that runs the engine stays on.

- A technology is chosen as mature, mainstream and fit for the project's scale, with no item-by-item comparison of its
  peers. Nobody would really make that comparison, and SQLite and React were not chosen that way.

- The part of the program that carries the state machine is called the engine. The name says what it does. Calling it
  wf-agents would not tell it from the compiler and the session manager, which are in the program too, and a name of its
  own is one more thing each reader must first learn.

- "Board" is no name of its own: what means the data is called the database, and what means what the owner looks at is
  called the owner's page. Named for what it is, it needs no explaining; with a name of its own, each reader must first
  work out which of the two it means.
