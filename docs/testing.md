# Testing

How wf-agents itself is kept right: the tests of its suite, layer by layer; each cell's words, tried on real works;
runs that bring rare cases about on purpose; an installation that shows every state of the page; and trials on the real
installation. How the engine runs a project's tests for its works is the engine's, in the decisions of its tools and
its host. The layers of the suite and the scripts that stand in for agents are built; what is not is said at the end
of each part.

## The layers of the suite

1. A test guards: it runs in CI at every change, the whole suite at the check before each merge and on each pull
   request that is not a work's, and fails on a fault of its kind from then on. What no test of the suite judges is
   judged otherwise: a cell's words on real works (10), the page by the owner (16 to 21), what rounds cost in trials
   (22 to 32). A look guards nothing until what it found is a test.
   How long each test and job of a merged check took is kept with its work, and a test's time and the CI's growth are
   judged by that history, not by a number set by hand (decision the-ci-grows-by-medians-of-its-merged-checks).

2. A fault, wherever it is found (by a review, in a trial, by the owner on the page), is first written as a test that
   fails on it, at the layer below that catches its kind, and only then fixed. Where that layer checks properties, it
   is written as a property, which says what the rule protects and so catches the fault's kin, not its one case. The
   test is written where the fault was seen (principle a-defect-is-seen-before-it-is-fixed). A fault of a cell's words
   is traced in forks of the round it showed in before the words change (27).

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
   every change, not by trying the flow once; a cell's words are tried apart (10), not its routing. A trial goes
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
   no-tool-opens-a-browser). A browser test of the page waits on a condition with `expect.poll`, never for a fixed time,
   and the bound of that wait and a test's own limit are set once, in the browser project of `client/vite.config.ts`:
   the machine that runs the tests runs other work too, so what a test waits for takes longer at times.

9. A test that fails and then passes on the same commit is a fault of the test or of the code, never noise: it is
   kept as an observation that becomes an item (decision a-red-the-change-did-not-make-is-not-sent-back).

## Each cell's words

10. A cell knows only its data (principle a-cell-knows-only-its-data). Which cell runs, and what it is given, in every
    situation is kept right by the suite (4, 5); how a model reads the words is not. It is tested on what real works
    bring: a round that went wrong in a trial is traced and mended in forks of its session (27), and the mended words
    are tried on the next real works. No cases are made for a cell, nor a project for them: on the made project of the
    first cases, 42 failed on faults of the made data rather than of the words, and its made CI passed a test that
    could only panic.

## Rare cases, brought about on purpose

11. A rare case, such as a failure of each kind, a usage limit and its reset, an answer that takes another option, a
    limit reached, the owner acting while a round is under way, the engine stopped between two steps, or a session
    lost or compacted, seldom comes in a real run, and when it comes it comes in a work that matters. The histories
    meet such cases at random (4); a script brings one about on purpose, with the real program, its store and its
    page, so that it can be looked at as well.

12. **A script in place of the agents.** The program runs with a script, and every agent, act of the engine and tool
    does what the script says, round by round, at no cost of a model. A round's return depends only on its flow, cell,
    run and round and the versions its run started from, so that a run stopped and started again goes on as it would
    have. Such a run shows what the engine and the page do with the case.

13. **What a script says**, for one case: what each round of each cell returns (what it hands in, asks or fails with,
    and what it takes); what each tool answers; what people and the outside do, each at a state of the work, such as
    an answer while the work waits for it, a pause while a round is under way, or the engine stopped at a step; and
    the path the run must take: the cells in their order, each wait and whom it waits for, and how the work ends.

14. **How its run is judged.** A program reads the run's records against the path its script states, and names the
    first step that left it, with what the records hold there. A script kept beside the flows whose agents are all
    stood in for is run by a test of the suite, so that a change of the flow it no longer fits fails at that change,
    not when someone next runs it. A rare case met in a real run that no script reaches becomes a script, and a fault it
    showed gets its test at its layer (2).

15. Not built: a script's acts of people and the outside, and its path; the program that judges a run by it; and the
    suite running the scripts kept beside the flows.

## Every state of the page

16. The owner judges every element of the page in each state it can show (Article 8), and a real run reaches most of
    those states seldom or never. So each is brought about on a test installation: a store of made-up items, apart from
    the real installation, served on a port of its own by the program of the change being judged, where every round
    follows a script.

17. Only the program writes the store. A build starts the engine on an empty data directory with a script, adds the
    items, and brings each work to its state with the engine's own commands and its interface, each act at the state it
    waits for. A state written into the store by hand may be one no engine reaches, and the page would be judged on it.

18. The states are taken from the flow's definition and the engine's own kinds, never from what was seen: under way at
    each cell; waiting, at each cell that can, for each thing it can wait for (an answer, a call's result, the engine's
    own question, an item it depends on); at each limit; paused by the owner, by the engine for each kind of failure,
    and by the pause of everything; stalled; merged by each way through the flow, and cancelled at each point; never
    let on; and the installation's own states, such as the wait for a service and every slot taken. A change that adds
    a state the page can show adds it to the build in the same work.

19. The build checks that each work reached its state, and writes the list of the states, each with the item that
    shows it, and each it could not bring about with why (principle nothing-passes-for-normal). The owner goes down the
    list on the test page in a browser, opening each state by its item. Each start serves a fresh copy of the store, so
    that what the owner does there is gone at the next, and a round left under way lasts hours, so that it stays under
    way.

20. The page's own made-up data, which it can show in place of the engine's, are for designing a view before the engine
    gives what it shows. They agree with themselves and not with the engine, so no change of the page is approved on
    them.

21. Not built: the script saying all the build needs (what a round does in one work alone, the questions a return asks
    as data, the engine's own questions, the context a round ends with, a tool's time in one work), so that the build
    runs the program as it is rather than one changed for it; the build kept in the repository beside the program it
    drives, as 25 needs, the items it fills the store with given from the installation's data (principle
    each-setting-where-it-is-true); and the states a script cannot bring about: sessions kept warm and what a round
    wrote to the cache, which only a trial shows (22); the settings and the levels, which a script runs without; the
    copies of the store; and times spread as a real installation's are, which needs the build to run on a clock of its
    own (principle outside-behind-interfaces).

## A trial on the real installation

22. A trial runs real works of the project on the real installation, for what no test, no case and no script shows:
    the cells of a work together, in the sessions their lines share and those sessions' cache; the host; what works
    cost; and the cells' words on what real works bring. Its works are real items that
    need doing; a made-up item belongs on the test installation (16), never on the real one.

### Before it

23. What is tried is merged first, and the installation runs a commit of main, so that what runs has passed the whole
    suite. For each change that goes in, an agent that did not make it lists every behaviour the change makes, each with
    the test that guards it, since the one that made it lists what it meant to do; a behaviour no test guards gets one
    before the trial, or, where only a model's reading shows it, is looked for in the trial. Beside each behaviour
    stands what in a real run would show it, which the trial looks for.

24. Before it is installed, a cell whose words changed has been through what principle
    change-cells-on-what-their-forks-bear-out asks (27).

25. A trial begins with one work and every other paused, and works go side by side only once the rounds of one show
    nothing new: a fault met by several works at once is paid for in each (Article 3).

### While it runs

26. A changed flow is tried one round at a time: before the next round runs, its inputs are read (their order, their
    size, and nothing given that the session already held), its return and route, and its token and cache numbers;
    then a fork asks about the cell's text (27). A hundred and one rounds run in a row once missed a problem that was
    there in the first; a round's numbers show an input given twice or a cache missed, which its return does not. Every
    trial goes so: the work is let on and paused again as soon as its round has begun, so that the round ends and the
    next one waits.

27. What an agent thinks of a cell is asked after its round, in a fork of its session, nothing of the work changed:
    `wf-agents fork` asks it through the engine, on the command line of the session's last round, every use of a tool
    refused (decision a-fork-keeps-the-sessions-tools); the round itself runs as usual, and the agent is not told it is
    being tested. The fork is asked only about the cell's text: which words can be read another way, what is missing or
    unneeded, how to fix each, quoting the text. An agent that knows it is being tested does not do what it would in
    real work (an inference, not measured). The return cannot show which words can be read another way; the fork,
    holding everything the round saw, can. Asking why it acted as it did is refused whole by the model's safety check
    (it happened on 2026-11-08). Each fork is sent a first message, the same for every fork of the round, before its
    question, so that the first fork writes the session's history to the prompt cache and the forks asked after it read
    it (decision a-fork-begins-with-a-message-every-fork-of-the-round-shares); what a fork cost is kept with the work,
    in what it spent.

    Where an agent did what its cell did not mean, the words are traced before they change. Each guess at which
    words led it there is written down first, with those words. A fork of its session, from the end of the round it
    went wrong in, is asked of each guess what those words told it and which sentence it went by, quoting it, the
    guess itself not told; and once more, what else it was given bore on it. Only a guess its answers bear out leads
    to a change. A fork from the same moment is then given the changed words and asked the same questions, and the
    words change again until it reads them as the cell means. Only then is the round given to a fork from the same
    moment as it would now read it, and it hands in the round's return again: the change stands where that return
    does what the cell means. In work 11, of three guesses at why an answer to the owner was put off for three
    rounds, the fork bore out two and showed the third wrong, which would have changed words that were not at fault.

28. What is read of each round is read where it shows: its inputs on the round's panel, as they were sent, and in its
    line's system prompt, against what its cell names and the order the flow declares (principle cache-order); its
    return, what was sent back and why, and the engine's reason for what runs next, on the work's page; what its agent
    reported in `observations`, in the Observations view; its numbers on the same panel, and each against the line of
    its own history (principle judged-by-own-history) in the data the page is given, with what went past those lines in
    the System data and in `wf-agents system`, none of it drawn on the page (decision
    what-went-past-its-line-is-given-in-data-only); and in the engine's log, what the engine met outside any work. The
    assistant reads the same through the commands that show a work (principle a-look-brings-back-little).

29. Every return sent back, and every way round between the same two cells after the first, is a finding (principles
    converging-fixes and report-all-at-once). Each finding is recorded at once, as an observation or an item. A fault of
    the engine or of the page holds the trial: the work stays paused until the fix is installed, and goes on from its
    records, not from its start (principle keep-what-was-paid-for). A cell's words change only as 24 says.

### After it

30. Each behaviour listed before is said as seen, with the round it showed in, by its work, run and round, or as not
    seen, with why; one not seen is brought about by a script (13), or stays said as unseen.

31. The trial's costs, by cell and in all, are set against the history of the flow's works. Once a work ends, each of
    its lines' sessions is forked, as 27 says, and asked about the rules it worked under.

32. Not built: a round's panel showing first what was new to it (document `the-owners-page`, 3); and walking a changed
    flow without acting (document `changed-flow-walks-the-records`, 3).
