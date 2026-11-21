# The flow

What every flow's definition holds, whichever flow it is, and how its files are laid out and compiled.

### decision the-flow-is-data-the-engine-compiles
A flow is data in files of wf-agents' own format: the engine's compiler checks them and the page draws the chart from them; the flow is in no workflow library's format.
- rejected: an agent proposed writing the flow with XState; the owner rejected it, because the flow's own table was enough, and the flow needed no nested states or parallel regions.
- rejected: an agent proposed drawing the changed chart from the Chinese source with the old generator, and copying each change into the repository's English definition; the owner rejected it, because the chart is to be drawn from the repository's definition, the one source.

### decision what-agents-are-sent-is-in-the-flows-files
Everything the engine sends agents is in the flow's files, never in the code or the installation's settings: its own words, the words its acts write, and the project's paths it names.
- rejected: an agent proposed keeping the paths of the project's guide, constitution and tree rules, and the owner's language, in the installation's settings; the owner rejected it, because he checks the flow's files for repetition, copies and hard-coding, and the page for what the files lack.
- replaced: Keeping in the engine's code the words that its acts write into blocks and commits was on main; the owner changed it, because what an agent is sent comes from the flow's files.

### decision the-flows-files-are-a-tree-of-concepts
The flow's files are laid out by concept, one thing to a file: the project's declaration, what every agent is sent, and each flow's lines, cells, blocks and tests.
- rejected: an agent proposed one file holding everything the flows share; the owner rejected it, because it held too much, including every line's definition.
- replaced: One directory for the engineer's flow, with all 41 of its blocks in its `flow.toml`, was on main; the owner changed it, because when each name has its own file, nothing can be defined twice, and a repetition shows in the file names.

### decision only-how-the-system-works-is-written-by-hand
Of what agents are told, only how the system works is written by hand, once for all flows; what each cell may return is built from the definitions of its line and of the cell.
- replaced: One hand-written file that mixed how to work, how to read a message and how to return, and gave each cell all its line could return, was on main; the owner changed it, because it held three things in one and copied what the definitions say.

### decision why-a-cell-exists-is-a-comment
Why a cell exists is a comment at the top of its file, for whoever changes the flow; it is never sent to the agent or added to its task.
- rejected: an agent proposed a required field `purpose` in each cell, shown first on its panel and as the first sentence of its task; the owner rejected it, because he told the assistant the reason for whoever changes the flow, and did not write it for the agent.
- rejected: an agent proposed a key `why` beside each datum a cell runs on or is given; the owner rejected it, because it is read by whoever changes the flow, not by the agent.

### decision a-cell-names-what-runs-it-apart-from-what-it-is-given
A cell's file lists separately the data that run it (`watches`), the data it is only given (`reads`) and the data it writes (`outputs`).
- replaced: One list of inputs for each cell, with a switch for the inputs not filled, was on main; the owner changed it, because these are two separate halves: the engine decides before a cell runs whether it runs, and after that the context manager decides what it is given.
- replaced: Keeping triggers apart from the data, with each cell naming in `triggered_by` what wakes it, was on main; the owner changed it, because with a dialogue for each party, the data are once again what runs the cells.

### decision a-cell-says-what-to-do-for-each-datum-it-watches
A cell's file says in `[do]` what its agent does when each datum it watches is new; a datum with no entry there is left to the cell's task.
- replaced: Saying in paragraphs of the task what to do when each datum is new was on main; the owner changed it, because what an agent does comes from its task and from what ran it: R0, for example, acts one way for a new background and another way for a new tree.

### decision conditions-are-values-of-blocks
A cell's `when` and `requires` list block values that must all hold, and a yes-or-no block states its `default`; a named test is used only where a condition reads several blocks at once.
- rejected: an agent proposed writing a condition as a block and a value the block does not have, such as `not = "yes"`; the owner rejected it, because it read badly, and the real cause was a yes-or-no block that is neither yes nor no before it is written.
- replaced: Using named tests such as `plan_needs_review`, read as an "or", was on main; the owner changed it, because a condition is a set of ticks on the rows of blocks, which must all hold together, as `settled` is.

### decision a-group-of-blocks-is-watched-as-one
A flow names groups of blocks, such as the plan, that a cell watches or reads as one: when any of them has a new version, the group is new to the cell once.
- replaced: Running a cell on each block of the plan separately was on main; the owner changed it, because a design review should start once the design is updated, and the design's writer hands in several blocks at once.

### decision a-datum-from-outside-says-where-it-comes-from
A datum written from outside the work says where it comes from in one key, `from`: the owner, or the source outside the work that writes it.
- replaced: Using two keys, `outside` naming a source and `people` for what people write, was on main; the owner changed it, because both come from outside the work, and the one difference the engine needs fits in one key.

### decision a-changed-by-with-no-source-is-refused
The compiler refuses a block that says who changes what its source writes, when no source outside the work writes that block.
- replaced: A key saying who changes what a block's source writes, on a block that no source outside the work writes, was on main; the owner changed it, because the compiler silently dropped the key.

### decision the-checks-words-are-its-own
What the check before the merge writes about the tests it runs again is in its act's file, `check.toml`, not in the file of the agents' tool `run_tests`.
- rejected: an agent proposed putting those words in the file of the tool `run_tests`; the owner rejected it, because they are the check's words, which an agent calling the tool never uses.

### decision every-agent-takes-is-the-flows
Besides what its own file names, every agent cell of a flow reads every cell's questions and words; these are named once, each with its kind, in `[every_agent]` of the flow's `flow.toml`.
- replaced: Naming the discussion and the questions in the context of each cell was on main; the owner changed it, because what every cell takes is part of the flow and is named once, so that the whole work knows what was asked, answered and said.

### decision a-datum-of-a-cell-s-own-is-made-where-a-cell-names-it
A per-cell or per-line datum, such as a cell's questions, is made for a cell or line only where some cell names it: in what it watches, reads or writes, or in its conditions.
- replaced: Making such a datum only where a cell names it in what it watches, reads or writes was on main; the owner changed it, because a datum named only in a condition, such as whether the better way was handed in, must exist too.

### decision a-cell-names-a-block-once
A cell takes each block by one name, the block's own or one group's; the compiler refuses a block that a cell takes by two names: alone and through a group, or through two groups.
- replaced: A cell naming a block both through its group and alone was on main; the owner changed it, because the block is then named twice with opposite meanings: watched, and only read.

### decision the-table-of-earlier-ids-is-a-list-of-renumberings
A flow's `earlier_ids` is a list of renumberings, oldest first, each naming by the ids they had the cells renumbered and every cell taken out, a stage's last too; it is only added to.
- rejected: an agent proposed a table of the last renumbering alone; the owner rejected it, because a version the engine never started on would leave its renumbering out of what a work on an earlier one is read through.
- rejected: an agent proposed giving each cell a name that never changes, so that no table is needed; the owner rejected it, because that changes principle cell-numbering, and the records kept name cells by their ids.

### decision a-renumbering-that-gives-a-held-id-is-refused
The compiler refuses a renumbering that gives an id of a stage it moves or takes out a cell of, at most the highest it moves there, without moving or taking out the cell that had it.
- rejected: an agent proposed refusing a renumbering that gives an id to a cell without saying where that id's cell went; the owner rejected it, because the compiler sees only the new version, so it cannot tell, and it would refuse an ordinary move of a cell to a free id.

### decision a-datum-made-for-a-cell-follows-its-cell
A datum made for each cell by a file of `data/of_a_cell/` is read through a renumbering as the one made for the id its cell takes; no table names it.
- rejected: an agent proposed reading only the cells through a renumbering, leaving the data made for them under their old names; the owner rejected it, because a renumbered cell's words and questions would then read as data no flow has.
- rejected: an agent proposed listing such data in the table of earlier ids; the owner rejected it, because that copies what the file that makes them says (principle one-thing-one-place).
