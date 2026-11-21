# The architect

The session that keeps a project's items: where it is started and what it is given, what it is responsible for, when it acts, and what it leaves to the owner.

### decision the-architect-starts-in-its-directory
An architect is a session started in `architect/` that reads the `AGENTS.md` written there by `assemble.py`: its rules and notes, the settings, and what main gives every agent.
- rejected: an agent proposed putting the help of `wf-agents` and of each of its commands into that file; the owner rejected it, because the architect can call the help itself.
- rejected: an agent proposed a shell script, `architect/assemble.sh`, to put the file together; the owner rejected it, because the file had to be put together, and written beside the script, from whatever directory the script is run in.

### decision one-architect-for-each-project
Each project has its own architect, put together for the project whose repository it starts in, and when one project needs something of another, the two architects deal with each other.
- replaced: One architect over every project was on main; the owner changed it, because one architect cannot scale as the projects grow.
- rejected: an agent proposed sending what one project asks of another in a fixed form through the engine's queues; the owner rejected it, because a fixed form makes talking hard.

### decision keeping-the-items-is-the-architects-duty
The architect is responsible only for the items, at all times: their dependencies, their order, their content and whether each starts or waits; all else it does serves that, and it writes no code.
- rejected: an agent proposed an architect whose duties start with the projects' direction, judging whether engineers are right, and deep technical questions; the owner rejected it, because the engineers now make the technical decisions of their work, so the architect no longer makes the key ones.
- rejected: the owner proposed a PM beside the architect, to keep the process and the status of the work; the owner rejected it, because the runner took over the PM's dispatching, waiting, merging and finishing.
- rejected: an agent proposed a standing observation manager, as a role of its own; the owner rejected it, because handling observations needs no role of its own.
- replaced: An architect that also kept its own understanding of the project, a plan separate from the items, and a policy agreed with the owner was on main; the owner changed it, because the architect is responsible for moving the items forward, and only for that.
- rejected: an agent proposed the architect's rules as a flat list, with none weighing more than another; the owner rejected it, because keeping the items is the architect's first duty and all else serves it, and the previous agent had only listed items at the root.

### decision the-architect-changes-items-unasked
The architect changes items without the owner's approval and says why with every change; the engine keeps the reason with the change for the owner to read afterwards.
- rejected: the owner proposed collecting every change the architect makes to the items and putting them to him, with the architect writing them only once he allowed it; the owner rejected it, because he reads the record of its changes afterwards.

### decision the-architect-keeps-the-items-as-things-happen
The architect updates the items as soon as each thing happens, waiting with `wf-agents wait` for what the engine reports, and never by tidying them up from time to time.
- rejected: the owner proposed an architect that reads all the work in progress carefully every hour and judges it by the principles; the owner rejected it, because tidying up at intervals cannot clear a backlog of thousands.

### decision nothing-to-start-is-told-to-the-owner
When a slot is free and no item can start, the architect tells the owner, and does not make an item just to fill the slot.
- rejected: an agent proposed that the architect then read the project again and plan afresh; the owner rejected it, because reading again when nothing has changed cannot change the answer, and an architect with nothing to do should tell him.

### decision a-red-main-comes-before-everything
A red main is an incident that comes before all else: the architect makes an item for it at once, lets it start ahead of all others and tells the owner, and the works it holds up depend on that item.
- replaced: An item for a red main, made like any other and placed under the items that wait for it, was on main; the owner changed it, because a red main is a serious incident that the architect must handle at once.
- rejected: an agent proposed taking away from the architect, along with its other extra duties, the choice between an item and a rollback when main goes red; the owner rejected it, because the architect moves the whole project forward by adding, deleting, changing, starting and pausing items.

### decision what-waits-for-a-person-is-the-owners
What the engine holds for a person belongs to the owner: the architect tells him about it and decides none of it, except a cell's question of how its work is to be split.
- rejected: an agent proposed letting the architect give, in the owner's place, what the engine holds for a person, such as the resume of every work after a restart; the owner rejected it, because those things are his, not the architect's.

### decision deliver-items-hold-the-architects-plan
Deliver items are the architect's own, and it keeps no other notes: its plan, each order the owner gave with its purpose, and the work due each time something happens are written in them.
- rejected: an agent proposed keeping each order the owner gave, with its purpose, in the architect's own notes in its directory; the owner rejected it, because the architect has no notes: all it has is in the items, and the deliver items are where it writes down and adjusts its plan.

### decision the-architects-rules-say-only-what-it-does
The architect's rules say only what the architect does, each in terms of the kind of thing it is about: they name no cell, give no sample case, and leave to the system what the system manages.
- rejected: an agent proposed lines on what may enter the tree, who writes a merge's title, that engineers leave an item's background alone, how works that collide are merged, and what an observation reports; the owner rejected it, because the system manages those.
- rejected: an agent proposed naming the deep research, by its id, as the cell that finds a work too large; the owner rejected it, because the rules must not hardcode a cell.
- rejected: an agent proposed putting sample cases in the rules, such as a parent item named after a compiler's back end; the owner rejected it, because that sample, a parent named after a class of things, is itself wrong.
