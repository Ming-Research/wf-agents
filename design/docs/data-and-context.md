# Data and context

What decides which cell runs next, and what decides what a round is given, are two things, and they are kept apart.
Which cell runs depends only on the data: on which of it is new and not yet consumed. What a round is given depends
only on what the context of its session already has. Neither reads what the other keeps.

Whatever a cell needs is brought under this as data with versions, and no second way is made where this one serves.
A cell knows nothing of the other cells: it watches its data, reads what it is given, and writes its outputs from them
alone (principle a-cell-knows-only-its-data). The document speaks of the data and of the context only. What else the
flow has stays as it is and is not said again here: the conditions a cell runs under, the limits, the owner's pauses,
when a work is at its end.

## The data

1. Everything a cell works from and everything it produces is data. Each piece of data has versions. A new version is
   added to the ones before it, and none is written over.

2. A new version comes from outside or from a cell. From outside: a person writes, the branch the works merge into
   moves, a file of the project that the cells work under changes (`AGENTS.md`, the design tree), a session is
   reopened. From a cell: what it hands in when a round of it ends. The code a round commits is handed in by that
   round, read from the branch as it ends.

3. A file's version is its content. It has a new version whenever its content changed, whoever changed it and
   wherever: on the branch the works merge into, or in the work itself.

4. The data carry themselves forward in this way: each new version takes the work from one state to the next. An agent
   is the tool of such a step, and nothing more.

## Which cell runs

5. A cell names three sets of data: those it watches, those it only reads, and those it writes, its outputs. A datum it
   watches runs it; a datum it only reads is given to it and runs nothing. Of each datum it watches the cell has
   consumed the versions up to one of them, and the datum is new to the cell while its latest version is later than
   that one. At a cell's first round nothing is consumed yet, and everything it watches is new.

6. The engine chooses by this. Going through the flow from its start, it takes the first cell that watches a datum new
   to it and that the flow lets run, and runs it for one round. Entering a cell means this: each round is one entering.

7. Nothing else runs a cell. There is no trigger apart from the data, no datum made only to run a cell, and the engine
   never sends a version to a cell by who wrote it (principle a-cell-knows-only-its-data). The engine knows nothing of
   sessions either: a session that is reopened reaches it only as a new version of data (24), and it chooses as always.

8. When the round ends, the versions it ran on are consumed, and are old to this cell from then on. A round that fails
   or is given up before its end consumes nothing. Data become old in no other way. A datum new to a cell the flow does
   not let run yet, its precondition not holding, stays new, and the cell runs once the precondition holds.

9. What the round hands in becomes new versions of the cell's outputs. They are new to the other cells that watch them,
   and not to the cell that wrote them. A round that hands in nothing new makes nothing new, and no cell runs because
   of it. The same holds for a version its writer hands in as changing nothing in meaning: it is kept, and is new to
   no cell. A change stops there.

10. Between two rounds of a cell a datum it watches may have gone on by several versions: two cells before it went back
    and forth, or people wrote more than once. What a cell last worked from is therefore the version it consumed, which
    need not be the one before the latest.

11. The engine tells each round which of the data it watches are new, and for each of them the version the cell last
    consumed and the latest. What the cell does when a datum it watches is new is said with that datum in the cell's
    definition, and the cell's task says how the agent brings its outputs up to date for it.

12. Data that a cell watches as one, such as the blocks of the plan, may be named together as a group: a new version
    of any of them is new to the cell, and the round deals with the group as one, by its name.

## The context manager

13. The context of a session holds data: what was put into it, what the agent fetched for itself, and what the cells
    that ran in it wrote. The context manager keeps, for each session, which versions of which data its context has.
    What a round hands in counts: it is in the context of the session the round ran in.

14. The context manager chooses no cell. Each time the engine runs a cell for a round, the manager compares the data
    the cell watches and reads with what the session has and fills in what is missing, each by its kind (17), in the
    order the cell names them.

15. A session that is reopened has nothing, and its record is emptied. The next round in it is filled with all that
    its cell's data ask for, from the top. What every cell is given alike comes first, `AGENTS.md` before all, so that
    sessions share it in the cache.

16. Whether a cell runs is never the manager's matter, and what is filled is never the engine's.

An example. The cell that implements writes a doubt about the plan, in the dialogue it holds with the plan's writer
(34), in a session they share. The doubt is new to the writer, which watches that dialogue, so the writer runs next. The
doubt is already in the session's context, since the session wrote it, so nothing is filled. Had the session been
reopened after the doubt was written, the writer would run next all the same, and the doubt would be filled, since the
new session lacks it.

## The three kinds of what a cell is given

17. Each datum a cell watches or reads is given in one of three kinds, which says what the context must hold of it when
    the cell runs.

    - **The latest.** Only the latest version.
    - **The last and the latest.** The version the cell last consumed, and the latest, so that the agent sees what
      changed between them.
    - **All of it.** Every version, as with a dialogue.

    At a cell's first round there is no version last consumed, and a datum of the second kind is filled as its
    latest. A datum the cell only reads has no version consumed, and is filled as its latest or all of it.

18. The kind belongs to a cell's datum, not to the data: two cells may take the same data in different kinds.

19. Where several data are new in one round, each is filled by its own kind, and none changes how another is filled.

20. The kind says what is filled. What the agent does with what is new is said with the datum and by the cell's task
    (11): to make an output anew, or to bring the last one up to date. An output brought up to date may be handed in
    whole.

21. A cell that brings an output up to date needs the latest version of that output, whoever wrote it. Where its
    session lacks it, as one reopened since does, the manager fills it in, as it fills in the version last consumed of
    a datum of the second kind.

22. How far a change travels follows from this. An output made anew is a new version each time, so the change reaches
    the cells after. A round that brings its outputs up to date may find that they stand as they are and hand in
    nothing new, and the change ends there (9). So when `AGENTS.md` changes, the cell of every agent runs again, and
    each does only this: it sees that `AGENTS.md` changed and nothing else, looks over its outputs for what the change
    touches, and where there is nothing, hands in nothing new.

## What an agent reads by itself

23. Not everything in a context is data. What an agent reads on its own account, the files of the design tree and
    the code, is in the context of the session that read it and nowhere else, and the context manager cannot fill it
    in. Of the tree it fills in the version, the commit and the id of the tree, and what changed since the version the
    session read, as the files that changed, so that the agent reads those again.

24. `AGENTS.md` can be put into a context as it is. So it is data, which the cell of every agent watches: the context
    manager fills it into each new session, and a cell runs again because of it only when its content changed.

25. The design tree is read by the agent, which chooses what bears on the problem. One cell does this reading for the
    cells that follow it in its session. So that the reading is done again in a session reopened, this cell watches
    `session`: data that stand for the session of a line, one for each line, and get a new version each time that
    session is reopened. Where a line has no cell whose work is only that reading, the first cell of the line reads the
    tree as it runs, wherever its session has not read it for the work. The tree runs no cell: it is a datum the cells
    that need it read, so that a changed tree is read where the line goes on, by the next cell that enters the session,
    and not by running the reading again (23). No data stand for the reading itself. What the cell finds for other
    cells it hands in as any cell does; run again for a new session alone, it finds nothing changed and hands in nothing
    new, so that no cell runs because of it (9).

26. A session reopened while a round is under way is no case of its own. `session` has a new version; the round did
    not end and consumed nothing (8); the engine takes the first cell with a new datum, which is the cell that reads,
    and after it the cell whose round did not end.

## Data that more than one cell may write

27. Some data may be written by more than one cell, as the plan is by the research and by the deep research. Which of
    them is in charge is decided before both: the cell that decides hands in the answer, and that answer is a
    condition of its own next round as it is of the other cell's. From then on only the cell in charge runs, and the
    way forks right where the two cells stand, not after one of them.

28. What another cell says to whoever is in charge is one datum, a dialogue (34), which each cell that may be in charge
    watches. The flow's condition says which of them acts on it. An agent is never left to choose where to write by who
    is in charge: where there is one place to write, nothing can be written to the wrong one.

29. A round is not told who wrote a version: nothing in this design needs it.

## Questions to the owner

30. A cell that may ask the owner a question has a datum of its own for its questions, which it writes and watches. A
    question asks for a choice of one or more among options, says what the cell recommends and whether the work waits
    for the answer (principle a-question-is-a-choice). The owner's answer is the option taken and nothing else, written
    as the next version of that datum; the cell did not write it, so it is new to the cell, which runs again. A session
    reopened is filled with the answer as with any datum.

31. An answer that takes the option the cell recommends, to a question the work does not wait for, changes nothing the
    cell goes by: it is kept as a version that changes nothing in meaning (9), new to no cell.

32. Every cell reads every cell's questions, so that what was asked and how it was answered is known to all; only the
    cell that asked watches its own. Whether a question stands with no answer to its latest version (`asked`) is worked
    out by the engine from all of them, and a cell that must not run while one stands, as the check before the merge,
    watches it. A cell asks a question again, or withdraws it, by writing its datum anew. A question the engine asks
    itself of what a round did, such as whether a change of a protected file may merge, is asked in the questions of
    the cell whose round did it.

## The owner's words

33. A question and the owner's words are two channels. A question is asked by a cell and answered by an option (30).
    The owner's words may be said at any time, wherever the work stands, and may go back and forth; words about a
    question are words, not its answer. A cell that may ask a question can be spoken to (principle
    who-asks-can-be-spoken-to): it has a datum of the owner's words of its own, which the owner and the cell write in
    turn, and which the cell watches; a flow in which a cell that may ask has none is refused as it loads.

    - Every cell reads every cell's words, so that what the owner says once is known to the whole work; only the cell
      spoken to watches its own, and runs.
    - The owner speaks to a cell, or to a line, whose words reach the cell that line has reached: the one running or
      holding the work on that line, or else the one of that line that ran last. The page shows the cell the words will
      reach, as the engine decides it; where no cell can take them they are not sent, and the page says so: they never
      go to another cell unsaid.
    - The cell answers at once and goes on with its work, its answer written as the next version of its words. What
      else ran the cell is still new, so the cell is entered again and goes on with the rest (9). A round that says it
      has dealt with the owner's words and writes no answer has its return sent back.
    - While a round of the cell spoken to is under way, the words are put into its session right after its next use of
      one of its program's own tools, so that they do not wait for the round's end. This is done for the owner's words
      and for nothing else. Words that did not reach the round before it ended are still new to the cell, and it runs
      again.
    - While a round of a cell later in the flow is under way, that round is told to stop, the cell spoken to runs, and
      the flow comes back to the cell that stopped: a round that stopped so has dealt with nothing (8), and its cell
      runs again.

## Dialogues between a maker and those who judge it

34. A cell that makes something, the plan or the change, is judged by cells that did not make it, and may be doubted
    by a cell that builds on it. Each that judges or doubts it speaks to the maker in a dialogue of its own: one datum
    that the two write in turn, each watching it (principle one-dialogue-for-each-source). The one judging writes its
    points; the maker writes its answer as the dialogue's next version. A version is new to the side that did not write
    it, so an answer wakes only the one it answers, and a point only the maker. No datum is made for an answer, and none
    holds the words of more than one source.

35. Each that judges also hands in its verdict, a choice of its own (passed or not), at every round; a gate before the
    merge reads the verdicts. Where the two sides of a dialogue do not agree, the owner may write the verdict in the
    judge's place.

36. What the maker makes is watched by each that judges it: a change of it is new to every one of them, which judges it
    again. An answer in a dialogue is new only to the one it answers.

37. The engine's own check before the merge judges the change and reads no answer. What it finds is a datum the maker
    watches, and the maker answers it only by what runs the check again: a new head, or a question to the owner.

## The next word

38. A dialogue is open while its judge's verdict is that the work does not pass. In an open dialogue the one woken owes
    the next word (principle the-one-woken-owes-the-next-word):

    - the maker, woken by a point, answers each point in the dialogue, done or why not, or changes what the judge
      watches;
    - the judge, woken by an answer or a change, hands in its verdict: a pass, which closes the dialogue and writes no
      point, or a fail, which comes with a new point.

    A doubt, which no verdict holds open, is answered in the same way, and is closed by its source writing no more.

39. The engine sends back a return that leaves an open dialogue owing on the one that handed it in: a maker woken by a
    point that answered none and changed nothing its judge watches; a judge whose fail brings no new point.

40. A maker woken by anything other than a point (what the item asks, the owner's words, an answer, a session
    reopened) that finds nothing to change hands in nothing, and no judge runs (9).

41. Two sides that each keep to their own come to the limit of the maker's rounds, and the owner rules: writes the
    verdict, or tells the maker what to do.

## The engineer flow, laid out by this design

42. The engineer flow, cell by cell, in the order of the walk. "Plan" is the group of `plan_owner`, `plan_agent`,
    `acceptance` and `tree_edits`; "change" the group of `code`, `title`, `decisions`, `evidence`, `tests` and `head`.
    Each cell that may ask also watches its own questions and its own words (30, 33), and every agent's cell reads
    every cell's questions and words; these are not repeated in the table.

    | Cell | Runs where | Watches | Reads | Writes |
    |---|---|---|---|---|
    | S1 Start | the items it depends on merged | `start` | | `place` |
    | R0 Understand the problem and the design tree | | `background`, `engineer_session` | `tree` | nothing |
    | R1 Initial research | no deep research | `background`, `plan_review`, `tree_review`, `plan_doubts` | `tree` | `deep_needed` every time, `found`, plan, answers in its dialogues, questions |
    | R2 Is there a better way | no deep research | plan | `background`, `tree` | plan, where it finds a better way; questions |
    | D1 Deep research | deep research | `background`, `found`, `deep_researcher_session`, `plan_review`, `tree_review`, `plan_doubts` | `tree` | plan, answers in its dialogues, questions |
    | D2 Is there a better way | deep research | plan | `background`, `tree` | plan, where it finds a better way; questions |
    | D3 Design review | every work | plan, `plan_review` | `background`, `found`, `tree` | `plan_ok` every time; points in `plan_review` where it fails. Reads the tree first where its session has not |
    | D4 Design tree review | every work | plan, `tree_review` | `background`, `tree` | `tree_ok` every time; points in `tree_review` where it fails |
    | I1 Implement | | plan, `plan_doubts`, `review`, `testing`, `check` | `tree` | change, answers in `review` and `testing`, doubts in `plan_doubts`, questions |
    | T1 Review | | change, `review` | plan, `tree` | `review_ok` and `needs_testing` every time; points in `review` where it fails |
    | T2 Adversarial testing | `needs_testing` is yes | `needs_testing`, `acceptance`, `testing` | change, plan | `tests`, `testing_ok` every time; points in `testing` where something stays red |
    | M1 Check before the merge | every verdict passed, every question answered | `head`, `main`, `asked` | | `checked`, `check` |
    | M2 Merge | `checked` is yes | `checked` | | `merged` |

    The flow of a deliverable item has one cell, V1 Deliver, which runs where the items it depends on have merged,
    watches `dependencies` and writes `delivered`.
