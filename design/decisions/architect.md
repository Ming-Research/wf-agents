# The architect

The session that keeps a project's items: where it is started and what it is given, what it answers for, when it acts, and what it leaves to the owner.

### decision the-architect-starts-in-its-directory
An architect is a session started in `architect/`, reading the `AGENTS.md` that `assemble.py` writes beside itself: its rules and notes, the settings, and what main gives every agent.
- rejected: an agent proposed putting the help of `wf-agents` and of each of its commands into that file; the owner rejected it, because the architect can call the help itself.
- rejected: an agent proposed a shell script, `architect/assemble.sh`, to put the file together; the owner rejected it, because the file was to be put together from whatever directory the script is run in, and beside the script.

### decision one-architect-for-each-project
Each project has an architect of its own, put together for the project whose repository it starts in, and the architects of two projects deal with each other for what one needs of the other.
- replaced: One architect over every project was on main; the owner changed it, because one architect cannot scale as the projects grow.
- rejected: an agent proposed that what one project asks of another go in a fixed form through the engine's queues; the owner rejected it, because a fixed form makes talking hard.

### decision keeping-the-items-is-the-architects-duty
The architect answers for the items alone, at all times: their dependencies, their order, their content and whether each starts or waits; all else it does serves that, and it writes no code.
- rejected: an agent proposed an architect whose duties open with the projects' direction, judging whether engineers are right and deep technical questions; the owner rejected it, because with the engineers taking the technical decisions of their work, the architect no longer takes the key ones.
- rejected: the owner proposed a PM beside the architect, keeping the process and the status of the work; the owner rejected it, because the runner came to do the PM's dispatching, waiting, merging and finishing.
- rejected: an agent proposed a standing observation manager, a role of its own; the owner rejected it, because handling observations needs no role of its own.
- replaced: An architect that also kept its own understanding of the project, a plan apart from the items and a policy set with the owner was on main; the owner changed it, because the architect is responsible for moving the items on, and only for that.
- rejected: an agent proposed the architect's rules laid flat, with no order of weight; the owner rejected it, because keeping the items is the architect's first duty and all else serves it, where the agent before had only listed items at the root.

### decision the-architect-changes-items-unasked
The architect changes items without the owner's approval, saying why with every change, which the engine keeps with the change for the owner to read afterwards.
- rejected: the owner proposed gathering every change the architect makes to the items and putting them to him, the architect writing them once he allowed it; the owner rejected it, because he reads the record of its changes afterwards.

### decision the-architect-keeps-the-items-as-things-happen
The architect keeps the items at once as each thing happens, waiting with `wf-agents wait` for what the engine reports, and never by tidying them up from time to time.
- rejected: the owner proposed an architect that every hour reads all the work in progress carefully and judges it by the principles; the owner rejected it, because tidying at intervals cannot cure a backlog of thousands.

### decision nothing-to-start-is-told-to-the-owner
Where a slot is free and no item can start, the architect tells the owner, and makes no item only to fill the slot.
- rejected: an agent proposed that the architect then read the project and plan afresh; the owner rejected it, because a reread with nothing changed cannot change the answer, and an architect with nothing to do should tell him.

### decision a-red-main-comes-before-everything
A red main is an incident before all else: the architect makes an item for it at once, lets it start ahead of every other and tells the owner, and the works it holds up depend on it.
- replaced: An item for a red main made like any other, under what waits for it, was on main; the owner changed it, because a red main is a serious incident that the architect must handle at once.
- rejected: an agent proposed taking off the architect, with its other extra duties, the choice between an item and a rollback when main goes red; the owner rejected it, because the architect moves the whole project on, by items added, deleted, changed, started and paused.

### decision what-waits-for-a-person-is-the-owners
What the engine holds for a person is the owner's: the architect tells him of it and decides none of it, but for a cell's question of how its work is to be split.
- rejected: an agent proposed letting the architect give in the owner's stead what the engine holds for a person, as the resume of every work after a restart; the owner rejected it, because those things are his, not the architect's.

### decision deliver-items-hold-the-architects-plan
Deliver items are the architect's own and it keeps no other notes: its plan, each order the owner gave with its purpose and the work due each time something happens are written in them.
- rejected: an agent proposed keeping each order the owner gave, with its purpose, in notes of the architect's own in its directory; the owner rejected it, because the architect has no notes: all it has is in the items, and the deliver items are where it notes and adjusts its plan.

### decision the-architects-rules-say-only-what-it-does
The architect's rules say only what the architect does, each in the kind of thing it means: they name no cell, give no sample case, and leave to the system what the system keeps.
- rejected: an agent proposed lines on what may enter the tree, who writes a merge's title, that engineers leave a background alone, how colliding works merge and what an observation reports; the owner rejected it, because the system manages those.
- rejected: an agent proposed naming the deep research, by its id, as the cell that finds a work too large; the owner rejected it, because the rules are not to hardcode a cell.
- rejected: an agent proposed sample cases in the rules, as a parent item named for a compiler's back end; the owner rejected it, because such a sample, a parent named for a class of things, is itself wrong.
