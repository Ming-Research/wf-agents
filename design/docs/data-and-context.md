# Data and context

What decides which cell runs next, and what decides what a round is given, are two things, and they are kept apart.
Which cell runs depends only on the data: on which of it is new and not yet consumed. What a round is given depends
only on what the context of its session already has. Neither reads what the other keeps. So a cell says the two
apart: its triggers, what runs it, and its context, the data its session must hold (5).

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

5. A cell names its triggers, what runs it; its context, the data it is given; and the data it produces, its outputs.
   The first two are named apart, each for one of the two things this design keeps apart. A trigger is an event,
   named as one, never by a datum's name: that a datum has a new version (`background_changed`, one rule for every
   datum), or that something happened that holds no content of its own, as the design tree changing, which the agent
   reads by itself (25), a session reopened, main moving, or the owner speaking to the cell or answering it (39, 35).
   These last are triggers only, defined apart from the data, and never in a context. A trigger says the data its
   agent reads for it, which the cell's context holds: the owner speaking runs the cell, and what was said is in
   `discussion`. Some data are only in a context: what the cell must hold and what asks nothing of it, as the record
   of what is said (38) and an output of its own that it brings up to date (21).

6. A trigger stands on a datum, or, of one with no content, on a version kept only to count what happened. Of each, a
   cell has consumed the versions up to one of them, and the trigger is new to the cell while a later version stands.
   The trigger itself has no version: to the agent it is only that it happened. At a cell's first round nothing is
   consumed yet, and each trigger whose datum has a version is new.

7. The engine chooses by this. Going through the flow from its start, it takes the first cell that something new runs
   and that the flow lets run, and runs it for one round. Entering a cell means this: each round is one entering.

8. The engine knows nothing of sessions. A session that is reopened reaches it only as a new version of data (25),
   and it chooses as always.

9. A round ends by naming, in `dealt_with`, the triggers that ran it which it has dealt with. Their versions are
   consumed, and are old to this cell from then on. A trigger it leaves out is still new, so the cell is entered
   again, until nothing new runs it: the round is told so. A round that fails, or that is told to stop, has dealt with nothing. Data become old in no other way.

10. What the round hands in becomes new versions of the cell's outputs. They are new to the other cells that they run,
    and not to the cell that wrote them. A round that hands in nothing new makes nothing new, and no cell runs because
    of it. A version its writer hands in as changing nothing in meaning is a new version all the same, and is filled
    into a session that lacks it as any version is (14); it only runs no cell. A change stops there.

11. Between two rounds of a cell a datum may have gone on by several versions: two cells before it went back and
    forth, or people wrote more than once. What a cell last worked from is therefore the version it consumed, which
    need not be the one before the latest.

12. The engine tells each round, last in its message, which of the cell's triggers are new: each by its name, with what
    happened where the engine has more to say of it (the files of the tree that changed), and what the cell does for
    it, written on the cell for each trigger (`do`), or on the trigger for every cell. A round that is not told what
    ran it does not know what to do: the cell's task says what the cell is for, its triggers what to do now. Versions
    are no part of a trigger's line; what changed of a datum, and from which version, is the context's to say (17). A
    trigger is told with its `do` the first time too: the task says what the cell is for, and may not say what a
    trigger asks of it, nor when it is dealt with.

## The context manager

13. The context of a session holds data: what was put into it, what the agent fetched for itself, and what the cells
    that ran in it wrote. The context manager keeps, for each session, which versions of which data its context has.
    What a round hands in counts: it is in the context of the session the round ran in.

14. The context manager chooses no cell. Each time the engine runs a cell for a round, the manager compares the data
    of the cell's context with what the session has and fills in what is missing, each by its kind (17), in the order
    the cell names them; of a datum new to the cell that the session holds already, it says only that, with the
    version the cell last worked from. It fills the context whether or not a datum of it ran the cell, and a trigger
    with no content fills nothing. A cell's task is
    among what it is given, the latest of it, last, after what it works from: so it is filled once in a session, and
    again only when the session is reopened or the flow changes the task. So is which work and which item the session
    is for, filled where the session lacks it.

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
      changed between them. Where the session holds the one last consumed, the latest is filled as what changed
      since, unless that would be longer than the whole.
    - **All of it.** Every version, as with what is said to a work.

    At a cell's first round there is no version last consumed, and a datum of the second kind is filled as its
    latest.

18. The kind belongs to the cell and the datum together, not to the data: two cells may be given the same data in
    different kinds.

19. Where several data are new in one round, each is filled by its own kind, and none changes how another is filled.

20. The kind says what is filled. What the agent does with what is new is said by the cell's task and its triggers
    (12): to make an output anew, or to bring the last one up to date. An output brought up to date may be handed in whole.

21. A cell that brings an output up to date has that output in its context, and the flow says so as of any datum:
    the manager fills it by its kind where the session lacks it, as in a session reopened since. Nothing is filled
    into a session that is not in its cell's context, its own outputs included. Having an output in its context does
    not have the output run the cell: its triggers are named apart (5).

22. How far a change travels follows from this. An output made anew is a new version each time, so the change reaches
    the cells after. A round that brings its outputs up to date may find that they stand as they are and hand in
    nothing new, and the change ends there (10).

## What a session begins with, and what an agent reads by itself

23. Not everything in a context is data. What an agent reads on its own account, the design tree and the code, is in
    the context of the session that read it and nowhere else, and the context manager cannot fill it in.

24. What every cell of a line is given alike, and what stays the same for as long as a session lasts, is no data: it
    is the system prompt of the line's session, which the flow writes once on the line (principle
    the-system-prompt-holds-what-a-line-shares). It holds the project's guide for its agents (`AGENTS.md`), the
    constitution, the rules of the design tree, the rules for the agents that the line takes, the engine's own
    words on working in it, and how a cell returns (31), in that order. The agent's program takes it as the session
    begins and keeps it: a change of one of them reaches a session only when it is reopened, and runs no cell.

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

## What a cell asks the engine for

27. What an agent needs from the engine, such as its records or a check, it asks for in its return: a call is data the
    round writes, under the name of the tool. The agent's own program reaches nothing of the engine, which may be on
    another machine. Which tools a cell may ask for, the flow says on the cell, and the chart shows them. How each tool
    is used is told to a cell that may ask for it, with what it returns (31); a return that asks for another is sent
    back.

28. A round that asks for a call waits for its result: it ends with what ran it not dealt with (9), and the engine
    enters the cell again only once every result it asked for is there. The result is data the cell is given, and it
    runs no cell. Of the results of its calls a cell is given the latest: those of the calls its last round asked for.

29. The engine keeps every call with its result in the work's records. A call that fails has a result that says so,
    and the cell reads it and decides what to do. A result too large to give whole is kept whole in the records, and
    the cell is given its beginning, with word that it was cut for its size, how large it is, and the query that gets
    the rest.

30. A return is of one of two kinds, and the form of the line's return holds both apart, so that the agent's program
    refuses one that mixes them: a return that asks the engine for calls, which holds the calls and may hold a summary
    (50), and nothing else; and a return that does the cell's work, which holds what it has dealt with (9), what it
    writes, and what is said to the owner. A return that asks for calls has dealt with nothing, so the cell is entered
    again with their results.

31. Each field of a return, each tool and each datum is defined once, its shape with its words: the engine's own
    fields and tools in the files beside the flows (51), a datum in the flow's definition where it is declared, what a
    cell may write on the cell, and the tools its cells may ask for on the flow. Every agent cell has each field of
    the engine's and each tool its flow names. All that is sent about returning is made from those definitions, by
    rules that name no cell and no datum, and is written nowhere by hand, in three parts:
    - the flow's form, the shape alone, given with every call: the engine's fields and the flow's tools, the same for
      every cell of the flow; the blocks a cell writes go in one field, `outputs`, under their names, which the form
      does not list (24, principle one-form-of-return-for-a-session);
    - in the line's system prompt, a part of its own after the engine's words: how a cell returns, the same for every
      cell of the flow: the two kinds of return, and each field and each tool with its words;
    - with the cell's task, where the session lacks it, the blocks this cell writes, each with its words, and only
      those; a cell that writes none is sent nothing here. The names it may say it has dealt with are what ran it, as
      the message gives them (9). The page shows the same for each cell, and the engine checks a return against it: a
      block that is not the cell's, a value that is not one of its choice's, a block it must hand in and did not, a
      name of what did not run it.

## What a person answers

32. A question a cell puts to the owner is data of its own, written by the cell, with its options, the one the cell
    recommends, and whether the cell waits for the answer. A text points to a question by its name at the place the
    question belongs, and the question is shown there; a question no text points to stands alone. Writing a
    question, writing it anew or withdrawing it gives no text a new version, so no cell runs again for a text
    because something was asked about it.

33. Each question has a name, which the cell gives it when it first writes it and which no other question of the
    work has, and versions of its own. A round writes only the questions it adds, writes anew or withdraws, each
    under its name; a question it does not name stands as it is, with its answer. A question written again under its
    name has a new version, and an answer given on an earlier version no longer answers it. However the text that
    points to a question is changed, the question and its answer stand.

34. What the owner answers is data of its own, apart from the question: one record for the work, to which the owner
    adds an entry for each answer, and no entry is changed. An entry names the question, the version of it that is
    answered, the option taken, and the owner's reason where one is given; the last entry for a question is its
    answer. The record runs no cell, and each cell that asks is given it. An answer is not something said to the
    work (37), and what is said about a question is not its answer: neither stands in for the other.

35. Where the cell does not wait, the work goes on by the option it recommends. What then runs the cell that asked
    is a datum of its own, with no content, as for what the owner says (39): it has a new version when the owner
    takes another option than the one the work goes by. An answer that takes the option the work goes by adds its
    entry and runs no cell. Which cells have such a datum, the flow says: those that may write a question that is
    not waited for. An answer that runs the cell that asked stops a round of a later cell that is under way, as the
    owner's words do (43); it is not put into a round under way.

36. A question that waits stops the work at the cell that asked it: the engine goes past that cell to no later one
    while the question stands unanswered. Any answer to it runs the cell again, by the datum of (35), since the work
    goes by none of its options; what ran the cell before is dealt with or not as the round said, as in any round.
    A cell that cannot go on without the owner asks so,
    with a question that stands alone and is waited for: there is no other way of waiting for the owner. The merge
    waits in the same way, until every question that stands is answered.

## What is said to a work

37. What the owner says to a work is not an answer to a question: it may be said at any time, wherever the work
    stands, and may go back and forth.

38. What the owner says and what the agents say back are kept as one record, each entry with who spoke, in the order
    they were said: the owner adds to it, and so do the cells that answer, and no entry is changed. The cells that can
    be spoken to are the research, the deep research and the implementation. Each of them is given the record, all of
    it (17), so that a session reopened holds everything that was said. The record runs none of them.

39. What runs a cell that is spoken to is a datum of its own, with no content, which has a new version when the owner
    speaks to that cell. The owner chooses a line: the engineer's, or the deep research's once it has begun. The cell
    spoken to is the one of that line's cells that the work has reached. Which cells of a line can be spoken to, the
    flow says.

40. The cell answers at once and goes on with its work. It hands in its answer as it hands in anything, and the
    answer is added to the record; with it the round says that it has dealt with the owner's words. What else ran the
    cell is still new, so the cell is entered again and goes on with the rest (9). No tool is needed for answering.

41. While a round of the cell spoken to is under way, the words are put into its session right after its next use of
    one of its program's own tools, so that they do not wait for the round's end. This is done for what the owner says
    and for nothing else. Words that did not reach the round before it ended are still new to the cell, and it runs
    again.

42. While the cell spoken to has no round under way and waits for the result of a call, the engine enters it for the
    words, and it goes on waiting for the result.

43. While a round of a cell later in the flow is under way, that round is told to stop, the cell spoken to runs, and
    the flow comes back to the cell that stopped: a round that stopped so has dealt with nothing (9), and its cell
    runs again.

44. A round that says it has dealt with the owner's words and has handed in no answer has its return sent back.

45. What the words ask that is not the cell's to change, the cell passes on in the block it already writes for the
    cell whose it is, and the flow goes on as for any such block.

## Data that more than one cell may write

46. Some data may be written by more than one cell, as the plan is by the research and by the deep research. Which of
    them is in charge is decided before both: the cell that decides hands in the answer, and that answer is a
    condition of its own next round as it is of the other cell's. From then on only the cell in charge runs, and the
    way forks right where the two cells stand, not after one of them.

47. What another cell says to whoever is in charge is one datum, which runs each cell that may be in charge. The
    flow's condition says which of them acts on it. An agent is never left to choose where to write by who is in
    charge: where there is one place to write, nothing can be written to the wrong one.

48. A round is not told who wrote a version of a block: nothing in this design needs it. In the record of what is said
    (38), who spoke is part of the entry.

## What a return says that is no data of the work

49. What an agent comes across beyond its work that is worth an item of its own, a defect or something that should
    change, it may write in its return as observations, each a text that says what it is and where. They are no data
    of the work: they run no cell, and no cell is given them. Each is kept as an observation of the installation, with
    the work, the cell and the round it came from, and shown as not yet read. Whether it is urgent, and whether it
    becomes an item, is judged by whoever reads it, not by the agent that wrote it.

50. A return may also say, in a sentence or two, what its round did and found, and a round told to stop how far it
    got. It is kept with the round for the owner to read, and like an observation it is no data of the work.

## Where what is sent is written

51. Every word an agent is sent, and every word the owner's page says of what it is sent, is in a file the flows'
    definitions can be read in, and the code that puts a message together holds none: no word, no name of a part, a
    field, a tool or a datum, and no path of the project. What every flow shares is written once beside the flows:
    the project's files and settings in `project.toml`; in `agents/`, what a session begins with and in what order
    (`session.toml`), the engine's own words, the rules for the agents, the return with each of its fields
    (`return/`), each tool (`tools/`), and the parts of a message with how a datum is put in by its kind
    (`message/`); the engine's own data, which a flow names among those it uses, in `data/`. A flow's directory holds
    its own: `flow.toml` for what is the flow's as a whole, and one file to each line, cell, datum and named test, in a
    directory of its kind. One thing is in one file, so that a thing defined twice is two files of one name, which is
    refused; and a datum each cell has its own of (as what runs a cell the owner spoke to) is defined once and named by
    the cell, not written again for each. The page names, for each part, the file it comes from.
