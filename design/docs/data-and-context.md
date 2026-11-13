# Data and context

What decides which cell runs next, and what decides what a round is given, are two things, and they are kept apart.
Which cell runs depends only on the data: on which of it is new and not yet consumed. What a round is given depends
only on what the context of its session already has. Neither reads what the other keeps.

Whatever a cell needs is brought under this as data with versions, and no second way is made where this one serves.
The document speaks of the data and of the context only. What else the flow has stays as it is and is not said again
here: the conditions a cell runs under, the limits, the owner's pauses, when a work is at its end.

## The data

1. Everything a cell works from and everything it produces is data. Each piece of data has versions. A new version is
   added to the ones before it, and none is written over.

2. A new version comes from outside or from a cell. From outside: a person writes, the branch the works merge into
   moves, a file of the project that the cells work under changes (`AGENTS.md`, the design tree), a session is
   reopened. From a cell: what it hands in when a round of it ends.

3. A file's version is its content. It has a new version whenever its content changed, whoever changed it and
   wherever: on the branch the works merge into, or in the work itself.

4. The data carry themselves forward in this way: each new version takes the work from one state to the next. An agent
   is the tool of such a step, and nothing more.

## Which cell runs

5. A cell names the data it takes, its inputs, and the data it produces, its outputs. Of each input the cell has
   consumed the versions up to one of them. An input is new to the cell while its latest version is later than that
   one. At a cell's first round nothing is consumed yet, and every input is new.

6. The engine chooses by this. Going through the flow from its start, it takes the first cell that has a new input and
   that the flow lets run, and runs it for one round. Entering a cell means this: each round is one entering.

7. The engine knows nothing of sessions. A session that is reopened reaches it only as a new version of data (24),
   and it chooses as always.

8. When the round ends, the versions it ran on are consumed, and are old to this cell from then on. A round that fails
   or is given up before its end consumes nothing. Data become old in no other way.

9. What the round hands in becomes new versions of the cell's outputs. They are new to the other cells that take them,
   and not to the cell that wrote them. A round that hands in nothing new makes nothing new, and no cell runs because
   of it. The same holds for a version its writer hands in as changing nothing in meaning: it is kept, and is new to
   no cell. A change stops there.

10. Between two rounds of a cell an input may have gone on by several versions: two cells before it went back and
    forth, or people wrote more than once. What a cell last worked from is therefore the version it consumed, which
    need not be the one before the latest.

11. The engine tells each round which of its inputs are new, and for each of them the version the cell last consumed
    and the latest. The cell's task says how the agent brings its outputs up to date for what is new.

## The context manager

12. The context of a session holds data: what was put into it, what the agent fetched for itself, and what the cells
    that ran in it wrote. The context manager keeps, for each session, which versions of which data its context has.
    What a round hands in counts: it is in the context of the session the round ran in.

13. The context manager chooses no cell. Each time the engine runs a cell for a round, the manager compares the cell's
    inputs with what the session has and fills in what is missing, each input by its kind (16), in the order the cell
    names them.

14. A session that is reopened has nothing, and its record is emptied. The next round in it is filled with all that
    its cell's inputs ask for, from the top. What every cell is given alike comes first, `AGENTS.md` before all, so
    that sessions share it in the cache.

15. Whether a cell runs is never the manager's matter, and what is filled is never the engine's.

An example. The cell that implements writes a note for the cell that researches, in a session they share. The note is
new to the researching cell, so that cell runs next. The note is already in the session's context, since the session
wrote it, so nothing is filled. Had the session been reopened after the note was written, the researching cell would
run next all the same, and the note would be filled, since the new session lacks it.

## The three kinds of input

16. Each input of a cell is of one of three kinds, which says what the context must hold of it when the cell runs.

    - **The latest.** Only the latest version.
    - **The last and the latest.** The version the cell last consumed, and the latest, so that the agent sees what
      changed between them.
    - **All of it.** Every version, as with what people say to a cell.

    At a cell's first round there is no version last consumed, and an input of the second kind is filled as its
    latest.

17. The kind belongs to a cell's input, not to the data: two cells may take the same data in different kinds.

18. Where several inputs are new in one round, each is filled by its own kind, and none changes how another is filled.

19. The kind says what is filled. What the agent does with what is new is said by the cell's task (11): to make an
    output anew, or to bring the last one up to date. An output brought up to date may be handed in whole.

20. A cell that brings an output up to date takes that output as an input too, and the flow says so as of any input:
    the manager fills it by its kind where the session lacks it, as in a session reopened since, and what the cell
    itself handed in is not new to it (9). What a cell does not take as an input is not filled into its session, its
    own outputs included.

21. How far a change travels follows from this. An output made anew is a new version each time, so the change reaches
    the cells after. A round that brings its outputs up to date may find that they stand as they are and hand in
    nothing new, and the change ends there (9). So when `AGENTS.md` changes, the cell of every agent runs again, and
    each does only this: it sees that `AGENTS.md` changed and nothing else, looks over its outputs for what the change
    touches, and where there is nothing, hands in nothing new.

## What an agent reads by itself

22. Not everything in a context is data. What an agent reads on its own account, the design tree and the code, is in
    the context of the session that read it and nowhere else, and the context manager cannot fill it in.

23. `AGENTS.md` can be put into a context as it is. So it is data, which the cell of every agent takes: the context
    manager fills it into each new session, and a cell runs again because of it only when its content changed.

24. The design tree is read by the agent, which chooses what bears on the problem. One cell does this reading for the
    cells that follow it in its session. So that the reading is done again in a session reopened, this cell takes
    `session`: data that stand for the session of a line, one for each line, and get a new version each time that
    session is reopened. The cell also takes the design tree, so that a change of the tree has it read again. No data
    stand for the reading itself. What the cell finds for other cells it hands in as any cell does; run again for a
    new session alone, it finds nothing changed and hands in nothing new, so that no cell runs because of it (9).

25. A session reopened while a round is under way is no case of its own. `session` has a new version; the round did
    not end and consumed nothing (8); the engine takes the first cell with a new input, which is the cell that reads,
    and after it the cell whose round did not end.

## What a person answers

26. What a person answers to a cell's question is data. The question is an output of the cell, and the answer gives it
    a new version. The cell takes it as an input: it runs again when it is answered, and a session reopened is filled
    with the answer as with any input.

## Data that more than one cell may write

27. Some data may be written by more than one cell, as the plan is by the research and by the deep research. Which of
    them is in charge is decided before both: the cell that decides hands in the answer, and that answer is a
    condition of its own next round as it is of the other cell's. From then on only the cell in charge runs, and the
    way forks right where the two cells stand, not after one of them.

28. What another cell says to whoever is in charge is one datum, which each cell that may be in charge takes. The
    flow's condition says which of them acts on it. An agent is never left to choose where to write by who is in
    charge: where there is one place to write, nothing can be written to the wrong one.

29. A round is not told who wrote a version: nothing in this design needs it.
