# Data and context

What decides which cell runs next, and what decides what a round is given, are two things, and they are kept apart.
Which cell runs depends only on the data: on which of it is new and not yet consumed. What a round is given depends
only on what the context of its session already has. Neither reads what the other keeps.

Whatever a cell needs is brought under this as data with versions, and no second way is made where this one serves.
A cell knows only its data (principle a-cell-knows-only-its-data). The document speaks of the data and of the context
only. What else the flow has stays as it is and is not said again here: the conditions a cell runs under, the limits,
the owner's pauses, when a work is at its end.

## The data

1. Everything a cell works from and everything it produces is data. Each piece of data has versions. A new version is
   added to the ones before it, and none is written over.

2. A new version comes from outside or from a cell. From outside: whoever made the item writes its background; the
   owner answers, speaks or rules; the owner lets the work on, first or after the item was reopened (`start`); the
   items the work depends on merge (`dependencies`); the branch the works merge into moves (`main`); a file of the
   project that the cells work under changes (`AGENTS.md`, the design tree); a cell's specification changes (design
   document `changed-flow-walks-the-records`); a session is reopened (25). Each of these says that something happened
   outside the work, and runs a cell only where a cell watches it. From a cell: what it hands in when a round of it
   ends. The code a round commits is handed in by that round, read from the branch as it ends (`head`). One datum is
   neither: `asked`, which the engine works out from the questions (33).

3. A file's version is its content. It has a new version whenever its content changed, whoever changed it and
   wherever: on the branch the works merge into, or in the work itself.

4. The data carry themselves forward in this way: each new version takes the work from one state to the next. An agent
   is the tool of such a step, and nothing more.

## Which cell runs

5. A cell names three sets of data: those it watches, those it only reads, and those it writes, its outputs. A datum it
   watches runs it; a datum it only reads is given to it and runs nothing. Of each datum it watches the cell has
   consumed the versions up to one of them, and the datum is new to the cell while a later version is new to it: one
   the cell did not write, and none of those that are new to no cell (9, 30, 31, 39). At a cell's first round nothing
   is consumed yet, and everything it watches is new.

6. The engine chooses by this. Going through the flow from its start, it takes the first cell that watches a datum new
   to it and that the flow lets run, and runs it for one round. Entering a cell means this: each round is one entering.

7. Nothing else runs a cell (principle a-cell-knows-only-its-data). The engine knows nothing of sessions either: a
   session that is reopened reaches it only as a new version of data (25), and it chooses as always.

8. When the round ends, the versions of what it says it has dealt with are consumed, and are old to this cell from then
   on; what it did not deal with stays new, and the cell is entered again for it. A round that fails or is given up
   before its end consumes nothing. Data become old in no other way: a datum new to a cell that the flow does not let
   run yet, its precondition not holding, stays new, and the cell runs once the precondition holds.

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

12. Data that a cell watches or reads as one, such as the blocks of the plan, may be named together as a group. A new
    version of any of them is new to a cell that watches the group, and the round deals with the group as one, by its
    name.

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
(36), in a session they share. The doubt is new to the writer, which watches that dialogue, so the writer runs next.
The doubt is already in the session's context, since the session wrote it, so nothing is filled. Had the session been
reopened after the doubt was written, the writer would run next all the same, and the doubt would be filled, since the
new session lacks it.

## The three kinds of what a cell is given

17. Each datum a cell watches or reads is given in one of three kinds, which says what the context must hold of it when
    the cell runs.

    - **The latest.** Only the latest version.
    - **The last and the latest.** The version the cell last worked from, and the latest, so that the agent sees what
      changed between them.
    - **All of it.** Every version, as with a dialogue.

    At a cell's first round there is no version it last worked from, and a datum of the second kind is filled as its
    latest.

18. The kind belongs to a cell's datum, not to the data: two cells may take the same data in different kinds.

19. Where several data are new in one round, each is filled by its own kind, and none changes how another is filled.

20. The kind says what is filled. What the agent does with what is new is said with the datum and by the cell's task
    (11): to make an output anew, or to bring the last one up to date. An output brought up to date may be handed in
    whole.

21. A cell that brings an output up to date needs the latest version of that output, whoever wrote it. Where its
    session lacks it, as one reopened since does, the manager fills it in, as it fills in the version last worked from
    of a datum of the second kind.

22. How far a change travels follows from this. An output made anew is a new version each time, so the change reaches
    the cells after. A round that brings its outputs up to date may find that they stand as they are and hand in
    nothing new, and the change ends there (9). So when `AGENTS.md` changes, the cell of every agent runs again, and
    each does only this: it sees that `AGENTS.md` changed and nothing else, looks over its outputs for what the change
    touches, and where there is nothing, hands in nothing new.

## What an agent reads by itself

23. Not everything in a context is data. What an agent reads on its own account, the files of the design tree and
    the code, is in the context of the session that read it and nowhere else, and the context manager cannot fill it
    in. Of the tree it fills in the version, the commit and the id of the tree, and what changed since the version the
    session read, as the files that changed, so that the agent reads those again; where the session has read none of
    it, it says so.

24. `AGENTS.md` can be put into a context as it is. So it is data, which the cell of every agent watches: the context
    manager fills it into each new session, and a cell runs again because of it only when its content changed.

25. The design tree is read by the agent, which chooses what bears on the problem. A session that has not read the tree
    for the work reads it in the first round of a cell that reads `tree` entered in it, before that round's own work
    (23). On a line with a cell whose work is only that reading, as the engineer line has, that cell does it for the
    cells after it, and so that it does it again in a session reopened it watches its line's `session`: data that
    stand for the session of that line, with a new version each time the session is reopened. Only such a line has
    them. The tree runs no cell: a changed tree is read where the line goes on, by the next cell that enters the
    session (23), and not by running the reading again. No data stand for the reading itself. What the reading cell
    finds for other cells it hands in as any cell does; run again for a new session alone, it finds nothing changed and
    hands in nothing new, so that no cell runs because of it (9).

26. A session reopened while a round is under way is no case of its own. The round did not end and consumed nothing
    (8). On a line that has `session`, it has a new version, and the engine takes the reading cell first and after it
    the cell whose round did not end; on another line, the cell whose round did not end runs again, and reads the tree
    first (25).

## Data that more than one cell may write

27. Some data may be written next by either of two cells, as the plan is by the research and by the deep research.
    Which of them is in charge is decided before both: the cell that decides hands in the answer, and that answer is a
    condition of its own next round as it is of the other cell's. From then on only the cell in charge runs, and the
    way forks right where the two cells stand, not after one of them.

28. What another cell says to whoever is in charge is a dialogue (36), which each cell that may be in charge watches;
    the flow's condition says which of them acts on it. An agent is never left to choose where to write by who is in
    charge: where there is one place to write, nothing can be written to the wrong one. The cell that takes charge from
    another also watches that cell's questions, so that an answer the other can no longer act on reaches the cell that
    can.

29. A round is told the line of the cell that wrote each version, or that the owner wrote it, and never the cell.

## Questions to the owner

30. A cell that may ask the owner a question has a datum of its own for its questions, which it writes and watches. A
    question asks for a choice and says whether the work waits for the answer (principle a-question-is-a-choice). The
    owner's answer is the option taken and nothing else, written as the next version of that datum. A version that
    asks, whoever wrote it, is new to no cell: an answer is new to the cells that watch the questions it answers, the
    cell whose questions they are and the one that took charge from it (28), and the one that acts follows the option
    taken. A session reopened is filled with the answer as with any datum.

31. An answer that changes nothing the cell goes by is kept, and is new to no cell (9): one that takes the option the
    cell recommends, to a question the work does not wait for; and one to a question as it was asked before it was
    asked anew (decision an-answer-to-an-earlier-version-is-kept).

32. A question the work waits for holds the work at the cell whose question it is, while no answer stands to the
    question as last asked: no later cell runs, and the cell is entered meanwhile only for the owner's words or answer
    (35) and for the results of calls a round of it asked for. A cell asks a question again, or withdraws it, by writing
    its datum anew.

33. Every agent's cell reads every cell's questions, all of them, so that what was asked and how it was answered is
    known to all. Whether a question stands with no answer to it as last asked is worked out by the engine from all of
    them, as `asked`. A cell that is not to run while one stands has that as its precondition, and watches `asked` so
    that an answer runs it again, as the check before the merge does.

34. A question the engine asks of a work, such as whether a change of a protected file may merge, is asked in the
    questions of the cell the flow names for it, one that can change the work as the answer asks and can be spoken to:
    in the engineer flow, the implementation. The work waits for it (32), and the engine recommends nothing. Where it
    is asked is the flow's, and never follows who made what it asks about (principle a-cell-knows-only-its-data).

## The owner's words

35. A question and the owner's words are two channels. A question is asked by a cell and answered by an option (30).
    The owner's words may be said at any time, wherever the work stands, and words about a question are words, not its
    answer. A cell that may ask a question can be spoken to (principle who-asks-can-be-spoken-to): it has a datum of
    the owner's words of its own, a dialogue with the owner (principle one-dialogue-for-each-party), which the owner
    writes each time the owner speaks and the cell writes in answer, and which the cell watches. A flow in which a cell
    that may ask has none is refused as it loads.

    - Every agent's cell reads every cell's words, all of them, so that what the owner says once is known to the whole
      work; only the cell spoken to watches its own, and runs.
    - The owner speaks to a cell, or to a line, whose words reach the cell that line has reached: the one running or
      holding the work on that line, or else the one of that line that ran last. The page shows the cell the words will
      reach, as the engine decides it; where no cell can take them they are not sent, and the page says so: they never
      go to another cell unsaid.
    - The cell answers at once and goes on with its work, its answer written as the next version of its words. What
      else ran the cell and was not dealt with is still new, so the cell is entered again and goes on with the rest
      (8). A round that says it has dealt with the owner's words and writes no answer has its return sent back.
    - While a round of the cell spoken to is under way, the words are put into its session right after its next use of
      one of its program's own tools, so that they do not wait for the round's end. This is done for the owner's words
      and for nothing else. Words that did not reach the round before it ended are still new to the cell, and it runs
      again.
    - While the cell spoken to has no round under way and waits, for the results of calls or for the answer to a
      question, the engine enters it for the owner's words or answer, and it goes on waiting.
    - While a round of a cell later in the flow is under way, that round is told to stop, the cell the owner's words or
      answer is new to runs, and the flow comes back to the cell that stopped: a round that stopped so has dealt with
      nothing (8), and its cell runs again.

## Dialogues between a maker and those who judge it

36. A cell that makes something, the plan or the change, is judged by agents' cells that did not make it, and may be
    doubted by a cell that builds on it. Each that judges or doubts it has a dialogue with the maker (principle
    one-dialogue-for-each-party), which the two write and both watch: the one judging or doubting writes its points,
    and the maker writes its answer as the dialogue's next version. So an answer is new only to the one it answers,
    and a point only to the maker (5, 9).

37. Each that judges hands in its verdict, a choice of its own (passed or not), at every round, and the check before
    the merge reads the verdicts. A doubt has no verdict.

38. Each judge watches what should make it judge again, besides its dialogue. The reviews of the plan watch the plan,
    and the review of the code watches the change. The adversarial testing watches only what asks for it, the
    review's word that the change needs testing and the conditions the work is accepted by: run on each version of the
    code, it found nothing in half its runs.

39. Where the two sides do not agree, the owner may rule. On a judge's dialogue the owner writes the verdict in the
    judge's place: the dialogue's points written before it are new to no cell from then on (9), and the verdict stands
    until something the judge watches is new to it; the judge then judges again, and raises nothing the owner ruled
    on. On a doubt the owner speaks to the maker or to the doubter (35): the points stay as they are, the maker
    answers them, by the owner's words where they were said to it, and the doubter, spoken to, answers the owner and
    may write its doubt anew or withdraw it.

40. The engine's acts, such as the check before the merge, read no words, and are no party to a dialogue. What the
    check finds, `check`, is a datum the maker watches, and the maker answers it only by what runs the check again: a
    new head, or a question the work waits for (decision an-act-is-answered-by-what-runs-it).

## The next word

41. A judge's dialogue is open while its verdict is that the work does not pass, and a doubt while its last version is
    a point. In an open dialogue the one woken owes the next word (principle the-one-woken-owes-the-next-word):

    - the maker, woken by a point, answers each point, done or why not; or changes what its judge watches; or puts the
      dispute to the owner as a question the work waits for;
    - the judge, woken by an answer or a change, hands in its verdict: a pass, which closes the dialogue and writes no
      point, or a fail, with the points that still stand.

    The doubter, woken by an answer, owes nothing: it writes the points that still stand, or writes no more. A dialogue
    of the owner's words is open while its last version is the owner's, and the cell owes its answer (35).

42. The engine sends back a return that plainly leaves the word owed on the one that handed it in: the maker's, woken
    by a point, that answers no point, changes nothing its judge watches and asks no question the work waits for; a
    judge's fail that brings no point. So too a maker's, woken by `check`, that leaves nothing that runs the check
    again (40). A return with a question the work waits for is not sent back for a block it leaves out (decision
    a-return-hands-in-what-its-cell-must-every-time).

43. A maker woken by anything else, such as what the item asks, an answer or a session reopened, that finds nothing to
    change hands in nothing, and no judge runs (9). Woken by the owner's words, it answers them (35).

44. Two sides that each keep to their own come to the limit of the maker's rounds, and the owner rules (39).

## The engineer flow, laid out by this design

45. The engineer flow, cell by cell, in the order of the walk. "Plan" is the group of `plan_owner`, `plan_agent`,
    `acceptance` and `tree_edits`; "change" the group of `code`, `title`, `decisions`, `evidence`, `tests` and `head`.
    A dialogue is named by the datum its two sides write. Every agent's cell watches `AGENTS.md` (24), and reads every
    cell's questions and every cell's words (33, 35); each cell that may ask also watches its own questions and its own
    words (30, 35); a cell that reads `tree` reads it first where its session has not (25). None of this is repeated in
    the table, and a blank under "Runs where" is every work.

    | Cell | Runs where | Watches | Reads | Writes |
    |---|---|---|---|---|
    | S1 Start | the items it depends on merged | `start` | | `place` |
    | R0 Understand the problem and the design tree | | `background`, `engineer_session` | `tree` | nothing |
    | R1 Initial research | no deep research | `background`, `plan_review`, `tree_review`, `plan_doubts` | `tree` | `deep_needed` every time, `found`, plan, answers in its dialogues, questions |
    | R2 Is there a better way | no deep research | plan | `background`, `tree` | plan, where it finds a better way; questions |
    | D1 Deep research | deep research | `background`, `found`, `plan_review`, `tree_review`, `plan_doubts`, R1's questions | `tree` | plan, answers in its dialogues, questions |
    | D2 Is there a better way | deep research | plan, R2's questions | `background`, `tree` | plan, where it finds a better way; questions |
    | D3 Design review | | plan, `plan_review` | `background`, `found`, `tree` | `plan_ok` every time; points in `plan_review` where it fails |
    | D4 Design tree review | | plan, `tree_review` | `background`, `tree` | `tree_ok` every time; points in `tree_review` where it fails |
    | I1 Implement | | plan, `plan_doubts`, `review`, `testing`, `check` | `background`, `tree` | `code`, `title`, `decisions`, `evidence`, `acceptance`, `head` by its commits; answers in `review` and `testing`; doubts in `plan_doubts`; questions, which hold the engine's (34) |
    | T1 Review | | change, `review` | plan, `background`, `tree` | `review_ok` and `needs_testing` every time; points in `review` where it fails |
    | T2 Adversarial testing | `needs_testing` is yes | `needs_testing`, `acceptance`, `testing` | `code`, `evidence`, `plan_agent`, `background` | `tests`, `head` by its commits, `testing_ok` every time; points in `testing` where something stays red |
    | M1 Check before the merge | no verdict stands at fail, `review_ok` is yes, and no question stands unanswered | `head`, `main`, `asked` | `code` | `checked`, `check` |
    | M2 Merge | `checked` is yes | `checked` | `title` | `merged` |

    The flow of a deliverable item has one cell, V1 Deliver, which runs where the items it depends on have merged,
    watches `dependencies` and writes `delivered`.
