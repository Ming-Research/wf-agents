# The tools

The tools an agent asks the engine for through its return, which tools there are, and what their results give the agent.

### decision tools-are-asked-for-in-the-return
An agent asks for a tool in its return; the engine runs it and gives the result to the same session, which goes on from it.
- rejected: an agent proposed tools as cells of the engine on the chart, each running what a return lists; the owner rejected it, because a tool only turns input into output and decides no route: it is given in the agent's configuration, not drawn.
- rejected: an agent proposed calling tools during the round, through an engine channel that the agents' program reaches; the owner rejected it, because once the return is back the round has ended with the intent to call a tool, and the engine only runs it and resumes the session.
- rejected: an agent proposed making the quick reads tools of the program, called during the round; the owner rejected it, because to the model both ways are one tool call, and of the costs claimed for a return only one barely held up.

### decision the-tools-are-query-and-run-tests
The engine's tools are `query` and `run_tests`; an agent does everything else with its own command line.
- replaced: A read-only `gh` tool was on main; the owner changed it, because what CI ran comes whole in the result of `run_tests`, other works' pull requests come through `query`, and public pages through the agent's own web tools.
- replaced: The tool `dependency_sources`, which put dependencies' sources into the worktree, was on main; the owner changed it, because no round used it, and nothing keeps an agent from reading a dependency's sources on the web itself.
- replaced: The tool `measure` was on main; the owner changed it, because many measurements were made to get a result, not to pass.
- replaced: The tool `decision_history`, which ran `git log --follow` on the decisions, was on main; the owner changed it, because no round used it, its words took 3.9K characters, and an agent's own git does the same.
- rejected: an agent proposed a tool `read` that gives a version of a datum or the change between two versions; the owner rejected it, because one query gives any version and the agent diffs two itself, and the store was meant to give each version that way.

### decision a-measurement-is-a-request-of-run-tests
A request of `run_tests` may name measurements: the project's measure workflow times them in one run, on the commit the work began from and on the work's commit, and the result gives each timing.
- rejected: an agent proposed no new capability, with performance measured by the project's own tests, which `run_tests` runs once on main's commit and once on the work's; the owner rejected it, because before and after would be two separate runs, not compared fairly on one machine in one run.

### decision the-flow-names-its-tools
The tools a cell may ask for are those its flow names in `flow.toml`, the same for every cell of the flow.
- replaced: Giving tools cell by cell, in each cell's file, was on main; the owner changed it, because then the part of what a session is sent that every cell shares no longer varies with the cell.
- rejected: an agent proposed one set of tools for every flow alike; the owner rejected it, because another flow beside the engineer's, such as a researcher's, would not have these tools.

### decision a-tools-result-is-no-datum
A tool's result is not a datum: the run that asked for it goes on with it, and it runs no cell.
- rejected: an agent proposed treating a tool's result as a new datum for the cell that asked for it; the owner rejected it, because what the cell asked for comes back within the same run, which is not a new run and wakes no other cell.
- rejected: an agent proposed a sentence in the design of data saying that a run's own results and send-backs continue that run; the owner rejected it, because that sentence was itself a new mechanism.

### decision tests-run-where-the-host-checks
`run_tests` runs the tests an agent names: on GitHub by the fast workflow, on the commit named or else the branch's last, pushed to a branch of its own; on a local host by the check command.
- rejected: an agent proposed letting an agent's tests run on the machine the agents run on; the owner rejected it, because the old system ran them only in CI.
- rejected: an agent proposed that agents build and test on a machine of their own; the owner rejected it, because a run in CI and a local one differ in nothing essential, and tests run locally fight the owner for the processor.
- rejected: an agent proposed running a work's named tests directly on the owner's build machine over SSH; the owner rejected it, because his computer is not always on the local network.
- rejected: an agent proposed leaving the push to the agent and taking only `gh` from it; the owner rejected it, because once GitHub is the engine's, everything done there is the engine's, including the push.
- rejected: an agent proposed naming the tool `check`; the owner rejected it, because `check` already names the engine's check of a return, the check before the merge and CI's format checks, and does not say what the tool does.
- replaced: Running only the branch's last commit was on main; the owner changed it, because a new test could be shown red on the code before the change only by pushing a commit that broke the branch, and 14 of 20 commits of one work were such demonstrations.
- replaced: Pushing the commit to the work's branch and opening its pull request for each run was on main; the owner changed it, because each push made GitHub run the full workflows on the pull request again, beside the run the engine asked for.
- replaced: Running the full workflows where nothing is named was on main; the owner changed it, because the full workflows are run once before the merge, and the passed tests were about 252 KB of one such result of 256 KB.
- rejected: an agent proposed pushing the work's branch and opening its pull request where the commit run is the branch's head, and a branch of its own for any other; the owner rejected it, because each run would still start another full run on the pull request, and there would be two ways to push.
- rejected: an agent proposed still running the full workflows where nothing is named and only leaving the passed tests out of the result; the owner rejected it, because every call would still be a full run on the owner's machine, against running the full workflows once before the merge.

### decision run-tests-names-the-files-git-does-not-track
Where no commit is named, `run_tests` on GitHub runs the last commit and names the untracked files before its result; a tracked file's uncommitted change refuses it. A named commit runs as it is.
- replaced: Refusing to run while the worktree held anything not committed, untracked files included, was on main; the owner changed it, because a cache a local command left stopped the tool though the commit was sound, and a forgotten new file is named instead.
- rejected: an agent proposed refusing an uncommitted change where a commit is named too; the owner rejected it, because the agent has said which commit to run, and a demonstration commit is made while the changes it is made from are still uncommitted.

### decision no-cell-runs-the-tests
No cell of a flow runs the tests a cell asks for: they stay a call within that cell's run.
- rejected: an agent proposed a cell of the engine that runs the tests each cell asks for, with the request and the result as data; the owner rejected it, because it was there only to show a cell's CI run on the page, and a row for each tool on the chart shows that just as well.

### decision every-project-meets-one-contract-for-its-tests
Every project meets one contract for its tests: JUnit results, one name for a test, and, with `[ci]`, a fast and a full workflow (taking a commit, a call and a test's stop), and a measure one if any.
- rejected: an agent proposed that each project describe its own CI (its workflows, and how they are dispatched, read and name their tests) and the engine run it as described; the owner rejected it, because the projects are all his, and any of them can be changed to one general, reasonable design.
- replaced: Matching a named test to a log's tests by its whole name, a `::` ending or its binary's name was on main; the owner changed it, because the name the results give a test is the name an agent asks for it by and the name the fast workflow filters by.
- replaced: The fast workflow running lint besides the named tests was on main; the owner changed it, because a fast run should only build and run the tests needed, and everything else goes to the full workflows, run once at the end.
- replaced: The fast workflow declared in the installation's settings was on main; the owner changed it, because the installation keeps only what belongs to the machine: its tokens and the runners it has.
- rejected: an agent proposed having no fast workflow, with every check running the whole suite, which took two or three minutes; the owner rejected it, because much work had gone into the fast CI, with a machine of his kept to run what an agent needs within half a minute.
- replaced: A contract of workflows that take only a commit, a call, tests and checks was on main; the owner changed it, because how long a test may run comes from the history of the tests' times, which only the engine holds.
- replaced: A declaration that may leave out the fast or the full workflow was on main; the owner changed it, because `run_tests` runs only the fast workflow and the check before the merge only the full ones, so a project without one cannot have its tests run or its work checked.

### decision each-workflow-runs-where-the-project-says
Each of a project's workflows runs on the runner its declaration names, and is not asked for when the installation has no such runner for the repository.
- replaced: The fast workflow running only on the owner's build machine was on main; the owner changed it, because GitHub's machines are now good enough as well.

### decision agents-read-the-records-by-one-query
An agent reads the engine's records through `query`: one read-only SQL query it asks for in its return, answered with rows as objects keyed by column name.
- rejected: an agent proposed that the engine give each round everything it might need from the records; the owner rejected it, because SQL is needed: an agent given a query, the store and a short description gets what it wants on the first try, while what the engine gives is a guess.
- rejected: an agent proposed a few fixed commands for reading the records; the owner rejected it, because fixed commands grow with every question, and a query reads more easily than commands wrapped around it.
- rejected: an agent proposed giving agents the item's data as files or commands; the owner rejected it, because the engine and the agents may be on different machines, and the way into the store is his to control.
- replaced: Giving cells the list of every item as an input was on main; the owner changed it, because the whole flow was to drop the list in favour of the tool, since a list of every item grows with the items.
- replaced: Giving a query's rows as one escaped JSON string was on main; the owner changed it, because the form belongs to the flow and the engine follows it: the engine once changed it on its own and stopped a work by disagreeing with the flow.

### decision the-stores-comments-are-what-agents-are-told
What `query` tells agents about the store is made from the comments in `db/schema.sql`; what the comments mark as hidden from agents is left out and cannot be read.
- rejected: an agent proposed a layer of views for agents, one for each need; the owner rejected it, because a view for each need is a guess, and many views will not fit in the context.
- rejected: an agent proposed no views and no description, with an agent reading `sqlite_master`; the owner rejected it, because an agent would then not know what the store holds and how, which a short text kept with the tables tells it.
- replaced: Giving the agents the whole of `db/schema.sql` as a rule was on main; the owner changed it, because it is over 20 KB of context and belongs in the tool.
- replaced: Every table and column open to agents was on main; the owner changed it, because a few columns hold this machine's paths, or raw text that may hold them.
- rejected: an agent proposed keeping `query` closed; the owner rejected it, because cells such as the research need it to look up earlier works.

### decision a-tools-result-is-kept-whole-and-given-cut
A tool's result is kept whole in the work's records; when a round is given it cut at the tool's limit, the cut names a file in the temporary directory that holds the whole result.
- replaced: Keeping a result that was too large only in a file on the engine's machine, and giving its path, was on main; the owner changed it, because an agent on another machine cannot read the engine's file, and he asked that the agent be told the result was too large.
- rejected: an agent proposed that an agent read the rest of a cut result from the records in parts by queries; the owner rejected it, because the results are hidden from agents and the store reads no value past 64 KB, while a file has no such limit.
