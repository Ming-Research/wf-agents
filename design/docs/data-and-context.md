# Data and context

What decides which cell runs next, and what decides what a round is given, are two things, and they are kept apart.
Which cell runs depends only on the data: on which of it is new and not yet consumed. What a round is given depends
only on what the context of its session already has. Neither reads what the other keeps. So a cell says the two
apart: the data that run it, and the data it is given (5).

Whatever a cell needs is brought under this as data with versions, and no second way is made where this one serves.
The document speaks of the data and of the context only. What else the flow has stays as it is and is not said again
here: the conditions a cell runs under, the limits, the owner's pauses, when a work is at its end.

## The data

1. Everything a cell works from and everything it produces is data. Each piece of data has versions. A new version is
   added to the ones before it, and none is written over.

2. A new version comes from outside or from a cell. From outside: a person writes, the branch the works merge into
   moves, the design tree changes, a session is reopened. From a cell: what it hands in when a round of it ends.

3. A file's version is its content. It has a new version whenever its content changed, whoever changed it and
   wherever: on the branch the works merge into, or in the work itself.

4. The data carry themselves forward in this way: each new version takes the work from one state to the next. An agent
   is the tool of such a step, and nothing more.

## Which cell runs

5. A cell names the data that run it, the data it is given, and the data it produces, its outputs. The first two are
   named apart, each for one of the two things this design keeps apart. Most of what a cell takes is on both sides: a
   new version of it runs the cell, and its content is given. Some data only run a cell: the design tree, which the
   agent reads by itself (25), and data that only say that something happened, as that a session was reopened or that
   the owner spoke to the cell (30). Some are only given: what the cell must hold and what asks nothing of it, as the
   record of what is said (29) and an output of its own that it brings up to date (21).

6. Of each of the data that run it, a cell has consumed the versions up to one of them. Such a datum is new to the
   cell while its latest version is later than that one. At a cell's first round nothing is consumed yet, and each of
   them that has a version is new.

7. The engine chooses by this. Going through the flow from its start, it takes the first cell that something new runs
   and that the flow lets run, and runs it for one round. Entering a cell means this: each round is one entering.

8. The engine knows nothing of sessions. A session that is reopened reaches it only as a new version of data (25),
   and it chooses as always.

9. A round ends by saying which of the data that ran it it has dealt with. Those versions are consumed, and are old
   to this cell from then on. What it has not dealt with is still new, so the cell is entered again, until nothing
   new runs it. A round that fails, or that is told to stop, has dealt with nothing. Data become old in no other way.

10. What the round hands in becomes new versions of the cell's outputs. They are new to the other cells that they run,
    and not to the cell that wrote them. A round that hands in nothing new makes nothing new, and no cell runs because
    of it. The same holds for a version its writer hands in as changing nothing in meaning: it is kept, and is new to
    no cell. A change stops there.

11. Between two rounds of a cell a datum may have gone on by several versions: two cells before it went back and
    forth, or people wrote more than once. What a cell last worked from is therefore the version it consumed, which
    need not be the one before the latest.

12. The engine tells each round which of the data that run its cell are new, and for each of them the version the cell
    last consumed and the latest. It tells this of data that have no content too, as the design tree and a session
    reopened: a round that is not told what ran it does not know what to do. The cell's task says, for each of the
    data that run it, what the agent does when it is new, and how it brings its outputs up to date.

## The context manager

13. The context of a session holds data: what was put into it, what the agent fetched for itself, and what the cells
    that ran in it wrote. The context manager keeps, for each session, which versions of which data its context has.
    What a round hands in counts: it is in the context of the session the round ran in.

14. The context manager chooses no cell. Each time the engine runs a cell for a round, the manager compares the data
    the cell is given with what the session has and fills in what is missing, each by its kind (17), in the order the
    cell names them. It fills what the cell is given whether or not that is what ran the cell.

15. A session that is reopened has nothing, and its record is emptied. The next round in it is filled with all that
    its cell is given, from the top. What every cell of the line is given alike is not filled: the session begins
    with it (24).

16. Whether a cell runs is never the manager's matter, and what is filled is never the engine's.

An example. The cell that implements writes a note for the cell that researches, in a session they share. The note is
new to the researching cell, so that cell runs next. The note is already in the session's context, since the session
wrote it, so nothing is filled. Had the session been reopened after the note was written, the researching cell would
run next all the same, and the note would be filled, since the new session lacks it.

## The three kinds

17. Each datum a cell is given is of one of three kinds, which says what the context must hold of it when the cell
    runs.

    - **The latest.** Only the latest version.
    - **The last and the latest.** The version the cell last consumed, and the latest, so that the agent sees what
      changed between them.
    - **All of it.** Every version, as with what is said to a work.

    At a cell's first round there is no version last consumed, and a datum of the second kind is filled as its
    latest.

18. The kind belongs to the cell and the datum together, not to the data: two cells may be given the same data in
    different kinds.

19. Where several data are new in one round, each is filled by its own kind, and none changes how another is filled.

20. The kind says what is filled. What the agent does with what is new is said by the cell's task (12): to make an
    output anew, or to bring the last one up to date. An output brought up to date may be handed in whole.

21. A cell that brings an output up to date is given that output, and the flow says so as of anything a cell is given:
    the manager fills it by its kind where the session lacks it, as in a session reopened since. Nothing is filled
    into a session that its cell is not given, its own outputs included. Being given an output does not have the
    output run the cell: what runs it is named apart (5).

22. How far a change travels follows from this. An output made anew is a new version each time, so the change reaches
    the cells after. A round that brings its outputs up to date may find that they stand as they are and hand in
    nothing new, and the change ends there (10).

## What a session begins with, and what an agent reads by itself

23. Not everything in a context is data. What an agent reads on its own account, the design tree and the code, is in
    the context of the session that read it and nowhere else, and the context manager cannot fill it in.

24. What every cell of a line is given alike, and what stays the same for as long as a session lasts, is no data: it
    is the system prompt of the line's session, which the flow writes once on the line (principle
    the-system-prompt-holds-what-a-line-shares). It holds the project's guide for its agents (`AGENTS.md`), the
    constitution, the rules of the design tree, the rules for the agents that the line takes, and the engine's own
    words on working in it, in that order. The agent's program takes it as the session begins and keeps it: a change
    of one of them reaches a session only when it is reopened, and runs no cell.

25. The design tree is read by the agent, which chooses what bears on the problem. One cell does this reading for the
    cells that follow it in its session. So that the reading is done again in a session reopened, `session` runs this
    cell: data that stand for the session of a line, one for each line, with no content, and with a new version each
    time that session is reopened. The design tree runs the cell as well, so that a change of the tree has it read
    again. No data stand for the reading itself. What the cell finds for other cells it hands in as any cell does;
    run again for a new session alone, it finds nothing changed and hands in nothing new, so that no cell runs because
    of it (10).

26. A session reopened while a round is under way is no case of its own. `session` has a new version; the round did
    not end and consumed nothing (9); the engine takes the first cell that something new runs, which is the cell that
    reads, and after it the cell whose round did not end.

## What a person answers

27. A question a cell puts to the owner is data of its own, written by the cell, with its options, the one the cell
    recommends, and whether the cell waits for the answer. It may say where in a text it belongs, and is shown
    there; it may also stand alone. Writing a question gives that text no new version, so no cell runs again for the
    text because something was asked about it. What the owner answers is data written on the question. An answer
    runs no cell; the cell that asked is given it (5). A cell that waits ends its round with what ran it not dealt
    with (9), and the engine enters it again only once the answer is there. A cell that cannot go on without the
    owner asks so, with a question that stands alone and is waited for: there is no other way of waiting for the
    owner. Where the cell does not wait, the work goes on by the option it recommends, and the merge waits until
    every question is answered. An answer that takes the recommended option does nothing more. An answer that takes
    another is also words of the owner to the cell that asked: it is added to the record, and the datum that runs
    that cell for the owner's words has a new version (30). So a question that is not waited for is written only by
    a cell that can be spoken to. What is said about a question is said to the work (28), and is not its answer.

## What is said to a work

28. What the owner says to a work is not an answer to a question: it may be said at any time, wherever the work
    stands, and may go back and forth.

29. What the owner says and what the agents say back are kept as one record, each entry with who spoke, in the order
    they were said: the owner adds to it, and so do the cells that answer, and no entry is changed. The cells that can
    be spoken to are the research, the deep research and the implementation. Each of them is given the record, all of
    it (17), so that a session reopened holds everything that was said. The record runs none of them.

30. What runs a cell that is spoken to is a datum of its own, with no content, which has a new version when the owner
    speaks to that cell. The owner chooses a line: the engineer's, or the deep research's once it has begun. The cell
    spoken to is the one of that line's cells that the work has reached. Which cells of a line can be spoken to, the
    flow says.

31. The cell answers at once and goes on with its work. It hands in its answer as it hands in anything, and the
    answer is added to the record; with it the round says that it has dealt with the owner's words. What else ran the
    cell is still new, so the cell is entered again and goes on with the rest (9). No tool is needed for answering.

32. While a round of the cell spoken to is under way, the words are put into its session right after its next use of
    one of its program's own tools, so that they do not wait for the round's end. This is done for what the owner says
    and for nothing else. Words that did not reach the round before it ended are still new to the cell, and it runs
    again.

33. While the cell spoken to has no round under way and waits for the result of a call, the engine enters it for the
    words, and it goes on waiting for the result.

34. While a round of a cell later in the flow is under way, that round is told to stop, the cell spoken to runs, and
    the flow comes back to the cell that stopped: a round that stopped so has dealt with nothing (9), and its cell
    runs again.

35. A round that says it has dealt with the owner's words and has handed in no answer has its return sent back.

36. What the words ask that is not the cell's to change, the cell passes on in the block it already writes for the
    cell whose it is, and the flow goes on as for any such block.

## Data that more than one cell may write

37. Some data may be written by more than one cell, as the plan is by the research and by the deep research. Which of
    them is in charge is decided before both: the cell that decides hands in the answer, and that answer is a
    condition of its own next round as it is of the other cell's. From then on only the cell in charge runs, and the
    way forks right where the two cells stand, not after one of them.

38. What another cell says to whoever is in charge is one datum, which runs each cell that may be in charge. The
    flow's condition says which of them acts on it. An agent is never left to choose where to write by who is in
    charge: where there is one place to write, nothing can be written to the wrong one.

39. A round is not told who wrote a version of a block: nothing in this design needs it. In the record of what is said
    (29), who spoke is part of the entry.
