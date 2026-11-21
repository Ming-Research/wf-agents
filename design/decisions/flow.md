# The flow

What every flow's definition holds, whichever flow it is, and how its files are laid out and compiled.

### decision the-flow-is-data-the-engine-compiles
A flow is defined as data in files of wf-agents' own form, which the engine's compiler checks and from which the page draws the chart; no workflow library's format holds it.
- rejected: an agent proposed writing the flow with XState; the owner rejected it, because a table of the flow's own was enough, and nested states and parallel regions were not needed.
- rejected: an agent proposed drawing a changed chart from the Chinese source with the old generator, each change copied into the repository's English definition; the owner rejected it, because the chart is then made from the repository's definition, the one source.

### decision what-agents-are-sent-is-in-the-flows-files
What the engine sends agents, its own words and what its acts write among it, and the project's paths it names, are in the flow's files, never in the code or the installation's settings.
- rejected: an agent proposed keeping the paths of the project's guide, constitution and tree rules, and the owner's language, in the installation's settings; the owner rejected it, because he checks the flow's files for repetition, copies and hard-coding, and the page for what they lack.
- replaced: The words the engine's acts write into blocks and commits, written in the engine's code, was on main; the owner changed it, because what an agent is sent comes from the flow's files.

### decision the-flows-files-are-a-tree-of-concepts
The flow's files are laid out by concept, one thing to a file: the project's declaration, what every agent is sent, and each flow's lines, cells, blocks and tests.
- rejected: an agent proposed one file holding everything the flows share; the owner rejected it, because it held too much, every line's definition among it.
- replaced: One directory for the engineer's flow, its 41 blocks in its `flow.toml`, was on main; the owner changed it, because with one name to one file a second definition cannot be made, and a repetition shows by the files' names.

### decision only-how-the-system-works-is-written-by-hand
Of what agents are told, only how the system works is written by hand, once for every flow; what each cell may return is made from the definitions, for its line and for the cell.
- replaced: One hand-written file that mixed how to work, how to read a message and how to return, and gave each cell all its line could return, was on main; the owner changed it, because it held three things in one and copied what the definitions say.

### decision why-a-cell-exists-is-a-comment
Why a cell exists is a comment atop its file for whoever changes the flow, never sent to the agent nor put with its task.
- rejected: an agent proposed a required field `purpose` in each cell, first on its panel and the first sentence of its task; the owner rejected it, because the reason was told to the assistant for whoever changes the flow, not written for the agent.
- rejected: an agent proposed a key `why` beside each datum a cell runs on or is given; the owner rejected it, because it is read by whoever changes the flow, not by the agent.

### decision a-cell-names-what-runs-it-apart-from-what-it-is-given
A cell's file names apart the data that run it (`watches`), those it is only given (`reads`) and those it writes (`outputs`).
- replaced: One list of inputs for each cell, with a switch for those not filled, was on main; the owner changed it, because whether a cell runs is decided before it runs, by the engine, and what it is given after, by the context manager, two halves apart.
- replaced: Triggers kept apart from the data, each cell naming in `triggered_by` what wakes it, was on main; the owner changed it, because with a dialogue for each party the data and what they run are one again.

### decision a-cell-says-what-to-do-for-each-datum-it-watches
A cell's file says in `[do]` what its agent does when each datum it watches is new; a datum with none is left to the cell's task.
- replaced: Paragraphs of the task saying what to do when each datum is new was on main; the owner changed it, because what an agent does comes from its task and what ran it, as the first reading acts one way for a new background and another for a new tree.

### decision conditions-are-values-of-blocks
A cell's `when` and `requires` list values of blocks that must all hold, a yes or no block saying its `default`, and a named test stands only where a condition reads several blocks at once.
- rejected: an agent proposed writing a condition as a block and a value it is not, as `not = "yes"`; the owner rejected it, because it read badly, its root being a yes or no block that is neither before it is written.
- replaced: Named tests such as `plan_needs_review`, read as an "or", was on main; the owner changed it, because a condition is ticks on the rows of blocks that all hold together, as `settled` is.

### decision a-group-of-blocks-is-watched-as-one
A flow names groups of blocks, such as the plan, that a cell watches or reads as one: a new version of any of them is new to it once.
- replaced: A cell run by each block of the plan apart was on main; the owner changed it, because a review of the design should start once the design is updated, and its writer hands in several blocks at once.

### decision a-datum-from-outside-says-where-it-comes-from
A datum written from outside the work says where it comes from in one key, `from`: the owner, or the source outside the work that writes it.
- replaced: Two keys, `outside` naming a source and `people` for what people write, was on main; the owner changed it, because both come from outside the work, and the one difference the engine needs fits one key.

### decision a-changed-by-with-no-source-is-refused
The compiler refuses a block that says who changes what its source writes, where no source outside the work writes it.
- replaced: A key saying who changes what a block's source writes, on a block no source outside the work writes, was on main; the owner changed it, because the compiler dropped it without a word.

### decision the-checks-words-are-its-own
What the check before the merge writes of the tests it runs again is in its act's file, `check.toml`, and not in the file of the agents' tool `run_tests`.
- rejected: an agent proposed putting those words in the file of the tool `run_tests`; the owner rejected it, because they are the check's words, which an agent calling the tool never uses.

### decision every-agent-takes-is-the-flows
What every agent cell of a flow reads beside what its own file names, every cell's questions and words, is named once in `[every_agent]` of the flow's `flow.toml`, each with its kind.
- replaced: The discussion and the questions named in the context of each cell was on main; the owner changed it, because what every cell takes is part of the flow, named once, so that what was asked, answered and said is known to the whole work.

### decision a-datum-of-a-cell-s-own-is-made-where-a-cell-names-it
A datum each cell or line has its own of, such as a cell's questions, is made only for a cell or line a cell names it by, in what it watches, reads or writes or in its conditions.
- replaced: Making such a datum only where a cell names it in what it watches, reads or writes was on main; the owner changed it, because a datum named only in a condition, as whether the better way was handed in, must exist too.

### decision a-cell-names-a-block-once
A cell takes each block by one name, its own or one group's; a block it takes by two, by itself and through a group or through two groups, is refused as the flow compiles.
- replaced: A block a cell named both through its group and alone was on main; the owner changed it, because the block is then said twice with opposite meanings, watched and only read.
