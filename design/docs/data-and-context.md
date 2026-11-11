# Data and context

What decides which cell runs next, and what decides what a round is given, are two things, and they are kept apart.
Which cell runs depends only on the data: on which of it is new and not yet consumed. What a round is given depends
only on what the context of its session already has. Neither reads what the other keeps.

## The data

1. Everything a cell works from and everything it produces is data. Each piece of data has versions. A new version is
   added to the ones before it, and none is written over.

2. A new version comes from outside or from a cell. From outside: a person writes, the branch the works merge into
   moves, a file of the project that the cells work under changes (`AGENTS.md`, the design tree), a session is
   reopened. From a cell: what it hands in when a round of it ends.

3. The data carry themselves forward in this way: each new version takes the work from one state to the next. An agent
   is the tool of such a step, and nothing more.

## Which cell runs

4. A cell names the data it takes, its inputs, and the data it produces, its outputs. Of each input the cell has
   consumed the versions up to one of them. An input is new to the cell while its latest version is later than that
   one.

5. The engine looks at nothing else. Going through the flow from its start, it takes the first cell that has a new
   input and runs it for one round. Entering a cell means this: each round is one entering.

6. When the round ends, the versions it ran on are consumed, and are old to this cell from then on. Data become old
   in no other way.

7. What the round hands in becomes new versions of the cell's outputs, which are new to the cells that take them. A
   round that hands in no new version makes nothing new, and no cell runs because of it. A change stops there.

8. Between two rounds of a cell an input may have gone on by several versions: two cells before it went back and
   forth, or people wrote more than once. What a cell last worked from is therefore the version it consumed, which
   need not be the one before the latest.

9. The engine tells each round which of its inputs are new, and for each of them the version the cell last consumed
   and the latest. The cell's task says how the agent brings its outputs up to date for what is new.

## The context manager

10. The context of a session holds data: what was put into it, and what the cells that ran in it wrote. The context
    manager keeps, for each session, which versions of which data its context has. What a round hands in counts: it is
    in the context of the session the round ran in.

11. The context manager chooses no cell. Each time the engine runs a cell for a round, the manager compares the cell's
    inputs with what the session has and fills in what is missing, each input by its kind (14), in the order the cell
    names them.

12. A session that is reopened has nothing, and its record is emptied. The next round in it is filled with all that
    its cell's inputs ask for, from the top. What every cell is given alike comes first, `AGENTS.md` before all, so
    that sessions share it in the cache.

13. Whether a cell runs is never the manager's matter, and what is filled is never the engine's.

An example. The cell that implements writes a note for the cell that researches, in a session they share. The note is
new to the researching cell, so that cell runs next. The note is already in the session's context, since the session
wrote it, so nothing is filled. Had the session been reopened after the note was written, the researching cell would
run next all the same, and the note would be filled, since the new session lacks it.

## The three kinds of input

14. Each input of a cell is of one of three kinds, which says what the context must hold of it when the cell runs.

    - **The latest.** Only the latest version. The cell makes its output anew, and what came before does not matter.
    - **The last and the latest.** The version the cell last consumed, and the latest. The cell brings its last output
      up to date for what changed between them; it may hand the output in whole.
    - **All of it.** Every version, as with what people say to a cell. The output is brought up to date as in the
      second kind.

15. The kind belongs to a cell's input, not to the data: two cells may take the same data in different kinds.

16. Where several inputs are new in one round, each is filled by its own kind, and none changes how another is filled.

17. A cell that brings an output up to date needs its own last version of it. The session that wrote it has it. In a
    session reopened since, the manager fills it in, as it fills in the version last consumed of an input of the
    second kind.

18. The kind decides how far a change travels. An input of the first kind has the cell make its output anew each
    time, so every change of it reaches the cells after. An input of the second or third kind lets the round find that
    its output stands as it is and hand in no new version, and the change ends there (7).

## What an agent reads by itself

19. Not everything in a context is data. What an agent reads on its own account, the design tree and the code, is in
    the context of the session that read it and nowhere else, and the context manager cannot fill it in.

20. `AGENTS.md` can be put into a context as it is. So it is data, which the cell of every agent takes: the context
    manager fills it into each new session, and a cell runs again because of it only when it was changed from outside.

21. The design tree is read by the agent, which chooses what bears on the problem. One cell does this reading for the
    cells that follow it in its session. So that the reading is done again in a session reopened, this cell takes
    `session`: data that stand for the session and get a new version each time it is reopened. The cell also takes
    the design tree, so that a change of the tree has it read again. It produces no output: it runs again and reads,
    and no cell after it runs because of it (7).
