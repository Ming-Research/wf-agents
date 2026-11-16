# Testing

How wf-agents itself is kept right: the tests of its suite, layer by layer; each cell tested on its data; runs that
bring rare cases about on purpose; an installation that shows every state of the page; and trials on the real
installation. How the engine runs a project's tests for its works is the engine's, in the decisions of its tools and
its host. The layers of the suite and the scripts that stand in for agents are built; what is not is said at the end
of each part.

## The layers of the suite

1. A test guards: it runs in CI at every change, the whole suite at every pull request and every push to main, and
   fails on a fault of its kind from then on. What no test of the suite judges is judged otherwise: a cell's words by
   its cases (10 to 17), the page by the owner (23 to 28), what rounds cost in trials (29 to 39). A look guards nothing
   until what it found is a test or a cell case.

2. A fault, wherever it is found (by a review, in a trial, by the owner on the page), is first written as a test that
   fails on it, at the layer below that catches its kind, and only then fixed. Where that layer checks properties, it
   is written as a property, which says what the rule protects and so catches the fault's kin, not its one case. The
   test is written where the fault was seen (principle a-defect-is-seen-before-it-is-fixed). A fault of a cell's words
   is written first as a cell case that fails on it (11 to 17).

3. **Each rule, by example.** Every part of the system (the deciding code, the engine's loop, the driver, the store,
   the context manager, the compiler, the carriers, the interface, the command and the page) has tests that each build
   a small case and say what holds of it as a whole, what each expects taken from the design tree and the part's own
   documentation, never from its code. This layer is the one to catch a rule built otherwise than the design says,
   and the one where each rule a change builds is first pinned.

4. **Random histories.** The engine is kept right by simulated histories run at every change: agents whose returns
   are drawn from all the flow allows, an owner who confirms, refuses, comments, pauses and cancels at any moment, an
   engine stopped at any step and a flow changed midway. After every step the properties the engine must always keep
   are checked. A fault found any other way is added as a history that shows it. An example tests the way its writer
   thought of. The two defects a review confirmed were in ways no scenario had been written for: a cell that asked for
   a tool after being sent back, and a work that went to design after it had implemented (5).

   Each part that keeps state over time (the deciding code, the loop, the driver, the store and the context manager)
   has histories of its own. A property is judged by the histories' own reading of the records, never by the code
   under test. Each such test counts every case it is there to reach and fails where no history reached one, so that
   no property passes by never being tried; a failing history gives its seed, which replays it. This layer is the one
   to catch a fault that needs an order of events nobody wrote down: a stop between two steps, a write while a round
   is under way, a return after a send-back.

5. **The flows as the repository defines them.** The flow's routing is kept right by tests in the suite, which run at
   every change, not by trying the flow once; a cell's cases judge its text (10 to 17), not its routing. A trial goes
   one way through the flow; the next change then meets a way no one ever ran.

   Each flow the repository defines is compiled with the files beside the flows, the system prompt of each of its
   lines and the form of its return are made, and every situation an entry of each of its cells can reach is written.
   The deciding code's own walk goes through it every way a work can go within a bound, or through many random
   histories, checking at each decision the properties that flow must keep, such as no merge on what is not settled
   and every work coming to rest. This layer is the one to catch a change of a definition (a cell, a block, a
   datum a cell watches, the words beside the flows) that opens a way nobody ran or sends what it should not. A
   fault of a flow's routing, wherever it was seen, is written here as a property of that flow.

6. **The edge with the outside.** The carriers are run against what the engine does not control: git, in a
   repository made for each test; a stand-in for the agents' command line that keeps what it was given and prints a
   return in its form; what that command line really printed, kept as it printed it; and the project's workflow
   files, as the engine relies on them. This layer is the one to catch a wrong belief about an outside program: the
   form of its output, what git does, a workflow's jobs and results. Such a fault gets a test with the program itself
   where CI can run it, and otherwise with what the program really printed, never with an imitation written from the
   belief that was wrong.

7. **The program whole.** The command runs as a command over a data directory of its own, its agents, acts and tools
   following a script, and the interface's routes are asked as the page asks them, over the engine's loop. This layer
   is the one to catch parts each right alone and joined wrong: a command that prints past its bound (principle
   a-look-brings-back-little), a route that gives the page other than what it reads, two flows handing a work between
   them.

8. **The page.** Its tests run in Node for what it works out from the data, in a stand-in for the browser for what a
   view draws from it, and in a real browser for what only a browser does: drawing, layout, the pointer, the keys and
   motion. A fault that shows only in a browser gets its test in a browser (principle
   a-defect-is-seen-before-it-is-fixed), which runs in CI, as no tool an agent has opens one (principle
   no-tool-opens-a-browser).

9. A test that fails and then passes on the same commit is a fault of the test or of the code, never noise: it is
   kept as an observation that becomes an item (decision a-red-the-change-did-not-make-is-not-sent-back).

## Each cell on its data

10. A cell knows only its data (principle a-cell-knows-only-its-data): what it hands in rests on its words, its data
    and what its session holds as it runs. So a model's reading of a cell's words is tested cell by cell, in the
    situations works bring it most, before a work meets them. Cells that each hand in what they owe there, on an
    engine whose choosing the suite guards (3 to 5), leave to a trial what only the cells together show.

11. **A cell's cases.** A cell case is one situation an entry of an agent's cell can reach (5), filled with data, and
    says what the cell owes in it. Each cell has a case for each of the twenty situations works bring it most: first
    those of a work's usual course, taken from the cell's definition (its first entry, its session holding what the
    cells before it on its line did, for each kind of work the flow serves, principle a-flow-serves-every-kind-of-work;
    then each datum it watches that the usual course makes new, alone: a point in each of a maker's dialogues, an
    answer and a changed product for a judge, an answer to a question, the owner's words), and then those real works
    reached most often. The rest, of the 1,376 situations the engineer flow's cells can be entered in, are left to the
    histories (4) and to trials: what a trial meets that no case held becomes a case (15).

12. **Its data.** Where a real work reached the situation, the case takes the versions it had, and its session as it
    was before that round began, forked from the agents' program's record of it. A situation no work has reached is
    a case only where it is a first entry, run in a new session on data made for it, in a repository made for the
    tests, for each project the flow serves and kept in that project's data directory, never in this repository
    (principle harness-only). A judgment is tested on data whose answer is known: a review on a plan that leaves a
    condition of the item unmet, which it must fail with a point that names it, and on one it must pass. A case and
    what it says the cell owes are made by an agent that did not write the cell's words, and reviewed as any change
    is.

13. **How it runs.** A program runs the cell's agent on each case for one round, on an installation apart from the
    real one, in a copy of the work's checkout at the case's versions, so that nothing it does reaches a project. It
    runs the real program on the cheapest model, so that every case of every cell can run at little cost, and with
    the cell's effort; the agent is not told it is tested (34). A round that asks for a call is given the result the
    case holds for it, and the case goes on to the round that hands in. Each case runs once, and one that fails runs
    twice more, so that a fault of the words is told from one that comes and goes: a case passes only where it never
    failed, and one that failed only sometimes is recorded as an observation as well (9).

14. **How it is judged.** First mechanically, by the engine's own checks of a return and against what the case says
    the cell owes: a return sent back, a block owed and left out, a verdict other than the case expects, a point where
    none is owed, or a version handed in as changing its meaning where what woke the cell asks no change, such as a
    judge woken by a change of `AGENTS.md` that does not touch what it judged (design document `data-and-context`, 41
    to 43), fails the case. Then, where only reading can judge, such as whether a point names what the case planted,
    by an agent given the case and the return. And a fork of each failed round's session is asked about the cell's
    text (34).

15. **A case that fails** shows words of the cell, or of what its line shares, that can be read otherwise; a
    definition that does not give the cell what it needs; or a case that expects wrongly. The words are rewritten
    (principle lean-agent-input), the definition or the case put right, and never an exception made for the case
    (principle a-cell-knows-only-its-data); where rewriting cannot stop the mistake, principle
    locate-and-price-the-fix says what may be added. Where the fork finds the words clear, the case runs on the model
    its cell runs at, to tell a fault of the words from what the cheapest model cannot do. A situation where a real
    run went wrong and no case held becomes a case.

16. **When it runs.** A cell's cases run where the text any of its situations is sent changed, as the situations
    written before and after the change show (5), and every cell's where the agents' program changed. They run in the
    work that makes the change, before it merges, and their results, each case passed or failed with its returns
    before and after the change side by side, are in its pull request for the owner (principle
    change-cells-after-their-cases). At twenty cases a cell, on the cheapest model, all the engineer flow's cases
    together cost a few dollars (an estimate, not yet measured).

17. Not built: the program that writes, runs and judges the cases, and the step of the flow that runs them in a work
    that changes a cell's text. Until they are, the cases are run by hand on the situations the compiled flow writes
    (5), and their results put in the work's pull request.

## Rare cases, brought about on purpose

18. A rare case, such as a failure of each kind, a usage limit and its reset, an answer that takes another option, a
    limit reached, the owner acting while a round is under way, the engine stopped between two steps, or a session
    lost or compacted, seldom comes in a real run, and when it comes it comes in a work that matters. The histories
    meet such cases at random (4); a script brings one about on purpose, with the real program, its store and its
    page, so that it can be looked at as well.

19. **A script in place of the agents.** The program runs with a script, and every agent, act of the engine and tool
    does what the script says, round by round, at no cost of a model. A round's return depends only on its flow, cell,
    run and round and the versions its run started from, so that a run stopped and started again goes on as it would
    have. Such a run shows what the engine and the page do with the case.

20. **What a script says**, for one case: what each round of each cell returns (what it hands in, asks or fails with,
    and what it takes); what each tool answers; what people and the outside do, each at a state of the work, such as
    an answer while the work waits for it, a pause while a round is under way, or the engine stopped at a step; and
    the path the run must take: the cells in their order, each wait and whom it waits for, and how the work ends.

21. **How its run is judged.** A program reads the run's records against the path its script states, and names the
    first step that left it, with what the records hold there. A script kept beside the flows whose agents are all
    stood in for is run by a test of the suite, so that a change of the flow it no longer fits fails at that change,
    not when someone next runs it. A rare case met in a real run that no script reaches becomes a script, and a fault it
    showed gets its test at its layer (2).

22. Not built: a script's acts of people and the outside, and its path; the program that judges a run by it; and the
    suite running the scripts kept beside the flows.

## Every state of the page

23. The owner judges every element of the page in each state it can show (Article 8), and a real run reaches most of
    those states seldom or never. So each is brought about on a test installation: a store of made-up items, apart from
    the real installation, served on a port of its own by the program of the change being judged, where every round
    follows a script.

24. Only the program writes the store. A build starts the engine on an empty data directory with a script, adds the
    items, and brings each work to its state with the engine's own commands and its interface, each act at the state it
    waits for. A state written into the store by hand may be one no engine reaches, and the page would be judged on it.

25. The states are taken from the flow's definition and the engine's own kinds, never from what was seen: under way at
    each cell; waiting, at each cell that can, for each thing it can wait for (an answer, a call's result, the engine's
    own question, an item it depends on); at each limit; paused by the owner, by the engine for each kind of failure,
    and by the pause of everything; stalled; merged by each way through the flow, and cancelled at each point; never
    let on; and the installation's own states, such as the wait for a service and every slot taken. A change that adds
    a state the page can show adds it to the build in the same work.

26. The build checks that each work reached its state, and writes the list of the states, each with the item that
    shows it, and each it could not bring about with why (principle nothing-passes-for-normal). The owner goes down the
    list on the test page in a browser, opening each state by its item. Each start serves a fresh copy of the store, so
    that what the owner does there is gone at the next, and a round left under way lasts hours, so that it stays under
    way.

27. The page's own made-up data, which it can show in place of the engine's, are for designing a view before the engine
    gives what it shows. They agree with themselves and not with the engine, so no change of the page is approved on
    them.

28. Not built: the script saying all the build needs (what a round does in one work alone, the questions a return asks
    as data, the engine's own questions, the context a round ends with, a tool's time in one work), so that the build
    runs the program as it is rather than one changed for it; the build kept in the repository beside the program it
    drives, as 25 needs, the items it fills the store with given from the installation's data (principle
    harness-only); and the states a script cannot bring about: sessions kept warm and what a round wrote to the cache,
    which only a trial shows (29); the settings and the levels, which a script runs without; the
    copies of the store; and times spread as a real installation's are, which needs the build to run on a clock of its
    own (principle outside-behind-interfaces).

## A trial on the real installation

29. A trial runs real works of the project on the real installation, for what no test, no case and no script shows:
    the cells of a work together, in the sessions their lines share and those sessions' cache; the host; what works
    cost; and the cells' words on what real works bring, which perhaps no cell case held. Its works are real items that
    need doing; a made-up item belongs on the test installation (23), never on the real one.

### Before it

30. What is tried is merged first, and the installation runs a commit of main, so that what runs has passed the whole
    suite. For each change that goes in, an agent that did not make it lists every behaviour the change makes, each with
    the test or cell case that guards it, since the one that made it lists what it meant to do; a behaviour no test
    guards gets its test before the trial. Beside each behaviour stands what in a real run would show it, which the
    trial looks for.

31. Before they are installed, a changed flow and a cell whose words changed have been through what principles
    flow-change-spares-works and change-cells-after-their-cases ask: the cell cases 16 runs pass. What an agent finds
    by reading a cell's words is made a cell case first, and the words change for a case that fails.

32. A trial begins with one work and every other paused, and works go side by side only once the rounds of one show
    nothing new: a fault met by several works at once is paid for in each (Article 3).

### While it runs

33. A changed flow is tried one round at a time: before the next round runs, its inputs are read (their order, their
    size, and nothing given that the session already held), its return and route, and its token and cache numbers;
    then a fork asks about the cell's text (34). A hundred and one rounds run in a row once missed a problem that was
    there in the first; a round's numbers show an input given twice or a cache missed, which its return does not. Every
    trial goes so: the work is let on and paused again as soon as its round has begun, so that the round ends and the
    next one waits.

34. What an agent thinks of a cell is asked after its round, in a fork of its session with no tools and nothing of the
    work changed; the round itself runs as usual, and the agent is not told it is being tested. The fork is asked only
    about the cell's text: which words can be read another way, what is missing or unneeded, how to fix each, quoting
    the text. An agent that knows it is being tested does not do what it would in real work (an inference, not
    measured). The return cannot show which words can be read another way; the fork, holding everything the round saw,
    can. Asking why it acted as it did is refused whole by the model's safety check (it happened on 2026-11-08).

35. What is read of each round is read where it shows: its inputs on the round's panel, as they were sent, and in its
    line's system prompt, against what its cell names and the order the flow declares (principle cache-order); its
    return, what was sent back and why, and the engine's reason for what runs next, on the work's page; what its agent
    reported in `observations`, in the Observations view; its numbers on the same panel, each against the line of its
    own history (principle judged-by-own-history), and in the System view what went past those lines; and in the
    engine's log, what the engine met outside any work. The assistant reads the same through the commands that show a
    work (principle a-look-brings-back-little).

36. Every return sent back, and every way round between the same two cells after the first, is a finding (principles
    converging-fixes and report-all-at-once). Each finding is recorded at once, as an observation or an item. A fault of
    the engine or of the page holds the trial: the work stays paused until the fix is installed, and goes on from its
    records, not from its start (principle keep-what-was-paid-for). A cell's words change only as 31 says.

### After it

37. Each behaviour listed before is said as seen, with the round it showed in, by its work, run and round, or as not
    seen, with why; one not seen is brought about by a script (20) or a cell case (11), or stays said as unseen.

38. The trial's costs, by cell and in all, are set against the history of the flow's works. Once a work ends, each of
    its lines' sessions is forked, as 34 says, and asked about the rules it worked under.

39. Not built: a round's panel showing first what was new to it (document `the-owners-page`, 3), and a work's cost by
    kind of token (the same, 4); asking a fork of a round's session about its cell's text, which is done by hand;
    and walking a changed flow without acting (document `changed-flow-walks-the-records`, 3).
