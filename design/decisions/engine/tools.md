# The tools

The tools an agent asks the engine for through its return, which of them there are, and what their results give it.

### decision tools-are-asked-for-in-the-return
An agent asks for a tool in its return; the engine runs it and gives the result to the same session, which goes on from it.
- rejected: an agent proposed tools as cells of the engine on the chart, each running what a return lists; the owner rejected it, because a tool only turns input into output and decides no route: it is given in the agent's configuration, not drawn.
- rejected: an agent proposed tools called within the round, through a channel of the engine's that the agents' program reaches; the owner rejected it, because once the return is back the round has ended with the intent to call a tool, and the engine only runs it and resumes the session.
- rejected: an agent proposed the quick reads as tools of the program called within the round; the owner rejected it, because to the model both ways are one tool call, and of the costs said of a return only one barely held.

### decision the-tools-are-query-and-run-tests
The engine's tools are `query` and `run_tests`; an agent does everything else with its own command line.
- replaced: A read-only `gh` tool stood on main; the owner changed it, because what CI ran comes whole in the result of `run_tests`, other works' pull requests through `query`, and public pages through the agent's own web tools.
- replaced: The tool `dependency_sources`, putting dependencies' sources into the worktree, stood on main; the owner changed it, because no round used it, and nothing keeps an agent from reading a dependency's sources on the web itself.
- replaced: The tool `measure` stood on main; the owner changed it, because many measurements were made to get a result, not to pass.
- replaced: The tool `decision_history`, running `git log --follow` on the decisions, stood on main; the owner changed it, because no round used it, its words took 3.9K characters, and an agent's own git does the same.
- rejected: an agent proposed a tool `read`, giving a version of a datum or the change between two; the owner rejected it, because one query gives any version and the agent diffs two itself, and the store was to give each so.

### decision the-flow-names-its-tools
The tools a cell may ask for are those its flow names in `flow.toml`, the same for every cell of the flow.
- replaced: Tools given cell by cell, in each cell's file, stood on main; the owner changed it, because the part of what a session is sent that every cell shares then no longer varies with the cell.
- rejected: an agent proposed one set of tools for every flow alike; the owner rejected it, because another flow beside the engineer's, such as a researcher's, would not have these tools.

### decision a-tools-result-is-no-datum
What a tool gives is no datum: the run that asked for it goes on with it, and it runs no cell.
- rejected: an agent proposed a tool's result as a datum new to the cell that asked for it; the owner rejected it, because what the cell asked for comes back within the same run, which is no new run and wakes no other cell.
- rejected: an agent proposed a sentence in the design of data saying that a run's own results and send-backs continue that run; the owner rejected it, because that sentence was itself a new mechanism.

### decision tests-run-where-the-host-checks
An agent has its tests run by asking for `run_tests`: on GitHub the project's workflows on the branch's last commit, which the engine pushes, and on a local host its check command.
- rejected: an agent proposed letting an agent's tests run on the machine the agents run on; the owner rejected it, because the old system ran them only in CI.
- rejected: an agent proposed that agents build and test on a machine of their own; the owner rejected it, because a run in CI and a local one differ in nothing essential, and run locally they fight the owner for the processor.
- rejected: an agent proposed running a work's named tests directly on the owner's build machine over SSH; the owner rejected it, because his computer is not always on the local network.
- rejected: an agent proposed leaving the push to the agent and taking only `gh` from it; the owner rejected it, because once GitHub is the engine's, everything done there is the engine's, the push included.
- rejected: an agent proposed naming the tool `check`; the owner rejected it, because `check` already names the engine's check of a return, the check before the merge and CI's format checks, and does not say what the tool does.

### decision no-cell-runs-the-tests
No cell of a flow runs the tests a cell asks for: they stay a call within that cell's run.
- rejected: an agent proposed a cell of the engine that runs the tests each cell asks for, its request and its result as data; the owner rejected it, because it was there only to show a cell's CI run on the page, which a row for each tool on the chart shows as well.

### decision every-project-meets-one-contract-for-its-tests
Every project meets one contract for its tests: results as JUnit XML, one name a test is asked for and filtered by, and a fast and a full workflow its declaration names.
- rejected: an agent proposed that each project describe its own CI, its workflows, how they are dispatched, read and name their tests, and the engine run it as described; the owner rejected it, because the projects are all his, and any can be changed to one general, reasonable design.
- replaced: Matching a named test to a log's tests by its whole name, a `::` ending or its binary's name stood on main; the owner changed it, because the name the results give a test is the one an agent asks for it by and the fast workflow filters by.
- replaced: The fast workflow running lint beside the named tests stood on main; the owner changed it, because a fast run should only build and run the tests needed, all else going to the full workflows, run once at the end.
- replaced: The fast workflow declared in the installation's settings stood on main; the owner changed it, because the installation keeps only what is of the machine: its tokens and the runners it has.
- rejected: an agent proposed no fast workflow, every check running the whole suite, which took two or three minutes; the owner rejected it, because the fast CI had been worked hard for, a machine of his kept to run what an agent needs within half a minute.

### decision each-workflow-runs-where-the-project-says
Each of a project's workflows runs on the runner its declaration names, and is not asked for where the installation has no such runner for the repository.
- replaced: The fast workflow taking only the owner's build machine stood on main; the owner changed it, because GitHub's machines are now good enough as well.

### decision agents-read-the-records-by-one-query
An agent reads the engine's records by `query`, one read-only SQL query it asks for in its return, answered with rows as objects by column name.
- rejected: an agent proposed the engine giving each round all it might need of the records; the owner rejected it, because SQL is needed: an agent given a query, the store and a short description gets what it wants at the first try, where what is given is guessed.
- rejected: an agent proposed a few fixed commands for reading the records; the owner rejected it, because fixed commands grow with every question, and a query reads more easily than commands wrapped around it.
- rejected: an agent proposed giving agents the item's data as files or commands; the owner rejected it, because the engine and the agents may be on different machines, and the way into the store is his to control.
- replaced: Giving cells the list of every item as an input stood on main; the owner changed it, because the whole flow was to drop the list for the tool, a list of every item growing with the items.
- replaced: A query's rows as one escaped JSON string stood on main; the owner changed it, because the form is the flow's, which the engine follows: the engine once changed it alone and stopped a work by disagreeing with the flow.

### decision the-stores-comments-are-what-agents-are-told
What `query` tells agents of the store is made from the comments of `db/schema.sql`, and what they mark hidden from agents is left out of it and cannot be read.
- rejected: an agent proposed a layer of views for agents, one for each need; the owner rejected it, because a view for each need is a guess, and many views will not fit in the context.
- rejected: an agent proposed no views and no description, an agent reading `sqlite_master`; the owner rejected it, because an agent would then not know what the store holds and how, which a short text kept with the tables says.
- replaced: The whole of `db/schema.sql` given to the agents as a rule stood on main; the owner changed it, because it is over 20 KB of context, and belongs in the tool.
- replaced: Every table and column open to agents stood on main; the owner changed it, because a few columns hold this machine's paths, or raw text that may hold them.
- rejected: an agent proposed keeping `query` closed; the owner rejected it, because cells such as the research need it to look up earlier works.

### decision a-tools-result-is-kept-whole-and-given-cut
A tool's result is kept whole in the work's records, and where a round is given it cut at its tool's bound, a file of the temporary directory the cut names holds it whole.
- replaced: A result too large kept only in a file on the engine's machine, its path given, stood on main; the owner changed it, because an agent on another machine cannot read the engine's file, and he asked that the agent be told the result was too large.
- rejected: an agent proposed that an agent read the rest of a cut result from the records by queries in parts; the owner rejected it, because the results are hidden from agents and the store reads no value past 64 KB, where a file has no such limit.
