# The design tree

What the design tree holds, how its entries are written and come to be, and the check that holds their forms.

### decision the-tree-records-what-the-code-loses
The design tree is the record kept beside the code of what was weighed against each choice, what was tried and why one way won, which the code does not keep.
- replaced: A system of derivations from the constitution, with facts, Open and Assumed marks and a check of its coherence, stood on main; the owner changed it, because agents can keep a memory of facts themselves, and facts hold no errors of logic.
- replaced: The owner's decisions kept as lines on the items they were settled in stood on main; the owner changed it, because an item's decision later overturned, recalled beside the decisions in force, only misleads.
- rejected: an agent proposed keeping the decisions in the engine's database, by project; the owner rejected it, because the database is apart from the code, and a directory beside the code keeps them with it.
- rejected: an agent proposed that the engine write the tree's files from the changes an agent hands in; the owner rejected it, because the tree is read freely as code is, not through a database, and it works without the engine.

### decision the-tree-holds-three-kinds
The tree holds three kinds: the constitution, the owner's principles for whatever is made later, and the decisions of what holds at one thing.
- replaced: Design documents as a fourth kind of the tree, each a design that holds only as a whole and may lead the code, stood on main; the owner changed it, because two sentences of the README on them pointed two ways, and they were cited by paragraph as decisions are yet passed no bar.
- rejected: an agent proposed facts as a kind of their own, listed with ids, the principles and decisions resting on them by links; the owner rejected it, because facts stay in the items, and decisions that match the code are checked against the code.
- rejected: an agent proposed intent, norm and limit as kinds of entry; the owner rejected it, because the three mix, agents turning his intents into norms, and the tree is kept to categories and decisions.
- rejected: an agent proposed that every principle and decision come from the constitution; the owner rejected it, because that says too much, and entries made to come from it make up their reasons.

### decision a-concept-holds-its-entries
Each directory is a tree of concepts that need not follow the code's layout, and an entry is written once, in the file of the lowest concept that holds all it concerns.
- replaced: One file of decisions for each ability or component, a decision of two written in one file and pointed to from the other, stood on main; the owner changed it, because a concept above holds what is common to several, and pointing lines went stale and were what agents missed most.
- rejected: an agent proposed laying the tree out as the code's tree; the owner rejected it, because by the code many things would sit very strangely.
- rejected: an agent proposed one file for each decision, its first line read by a search; the owner rejected it, because agents cannot read thousands of decisions, and a hierarchy of categories, several decisions to a file, finds what is needed in log n.
- rejected: an agent proposed links between entries that the engine follows, and an index of the tree it builds; the owner rejected it, because three earlier reorganisations of the tree failed from complexity and corner cases.
- replaced: The tree's root named for the project, beside the directory of its concepts, stood on main; the owner changed it, because the tree's directory is already in the project.

### decision a-file-of-the-tree-is-plain-text
A file of the tree is Markdown: its title, one line of what its concept covers, and its entries one blank line apart, each line of an entry one line of the file.
- rejected: an agent proposed the tree as structured data, JSON, TOML or YAML, with a view rendered for reading; the owner rejected it, because every work reads and writes the tree, which is handier as prose, and tools parse it rarely.

### decision a-decision-says-only-what-main-does
A decision's first line says only what the code does on main, while a principle may come before the code.
- replaced: A tree ahead of the code, a difference being code behind it that an agent mends or reports, stood on main; the owner changed it, because decisions that match the code are easy to keep consistent and to check against it.
- replaced: A file at the root of the repository holding the designs not yet true of main stood on main; the owner changed it, because there is no pending design, and a design ahead of the code is a document, which may lead it.
- rejected: an agent proposed a line on each entry saying what the code does not do yet, and which item will do it; the owner rejected it, because a status kept on entries drifts from reality.
- rejected: an agent proposed a rule that the whole tree hold only what the code on main does; the owner rejected it, because principles give direction, and so run ahead of the code.

### decision a-principle-is-the-owners-rule-for-a-kind
A principle is a rule for every thing of a kind, those not yet made included; an agent may propose one or a change to one, and the owner decides.
- replaced: Only the owner adding or changing a principle, an agent saying where one is missing or wrong, stood on main; the owner changed it, because an agent that fully understands the principles can help keep them, or at least propose a fix.
- replaced: A principle admitted only where two places that share no code already use it stood on main; the owner changed it, because a decision explains a thing that is there, while a principle binds what is not yet made.
- rejected: an agent proposed as a principle a rule for one place of the code; the owner rejected it, because a rule that governs one place is too narrow to be a principle.
- rejected: an agent proposed that a principle be one every agent working in its scope needs; the owner rejected it, because who reads a principle is a criterion of its use, not of what enters.

### decision a-principle-is-its-rule-and-its-why
A principle is its rule in one or two sentences of at most 300 characters and its reason after `Why:`, of at most 300 characters.
- rejected: an agent proposed writing a principle as When, Do and Why lines; the owner rejected it, because When and Do split one thing and read badly.
- rejected: an agent proposed a Rests on line naming what a principle rests on; the owner rejected it, because 26 of 28 principles had one without telling agents more.

### decision a-decision-comes-from-a-choice-put-to-the-owner
A decision comes only from a choice put to the owner: a way against a decision, two or more ways that would each do, or a change of an important mechanism no decision holds.
- replaced: A decision entering where an agent that read the code and the principles would still change it wrongly stood on main; the owner changed it, because such a bar asked of agents in their prompt failed, agents copying the entries beside them and inventing.
- replaced: Decisions entering by the work that makes them true without the owner's yes, which only principles waited for, stood on main; the owner changed it, because questions and the tree become one, so the tree's gate is the gate of questions.
- rejected: the owner proposed MCTS-Mem's gate, where only what a change modifies enters; the owner rejected it, because it kept out the choices weighed in research.
- rejected: the owner proposed Whitefoot's check that each decision say "because" or "instead of"; the owner rejected it, because it let in too much and made agents invent ways to write.
- rejected: an agent proposed a second line, "Not knowing this, an agent would", telling what an agent would do wrong without the entry; the owner rejected it, because it is no good gate.
- rejected: the owner proposed that a question which stops the work enter, and one that does not only where he takes another way than the one recommended; the owner rejected it, because whichever way he takes, the question and the tree are one.
- replaced: Leaving out of the tree what the code shows, what a thing does, its names and a property a test holds, stood on main; the owner changed it, because seeing what the code does is not understanding why it does it.
- rejected: an agent proposed that what a check enforces leave the tree for the check; the owner rejected it, because a check is code too, which an agent not knowing what it checks may change.
- rejected: an agent proposed entering designs once the code made them true; the owner rejected it, because being built is no ground to enter, and each must meet the bar.
- rejected: an agent proposed entries for things settled with no other way weighed, as the look of a label; the owner rejected it, because trifles and boring decisions do not enter the tree.
- rejected: an agent proposed a sentence apart from the kinds on what is not written; the owner rejected it, because whether a thing enters is said in the definitions of a principle and a decision, and nowhere else.

### decision a-choice-weighs-ways-that-would-each-do
Two ways make a choice only where each would do and each was researched or tried in earnest; a way plainly better is taken unasked, unless an experiment showed it.
- replaced: A card of at least three viable ways, never doing against not doing, stood on main; the owner changed it, because often only two ways are real, and forcing a third makes up options nobody would pick.
- rejected: an agent proposed keeping a list of the ways looked at and found to be no option; the owner rejected it, because a way that is no option has no value, and one explored in earnest is an option turned down.
- rejected: an agent proposed that a way with pros and cons of its own makes a choice; the owner rejected it, because a choice is a trade-off between the ways, and a way beaten in every respect is none.
- rejected: a reviewer agent proposed that a choice is real only where a changed premise would turn a rejected way into the chosen one; the owner rejected it, because a choice weighs the ways' pros and cons whether or not any premise changes.
- rejected: an agent proposed counting a way that breaks an article of the constitution as an option with a cost; the owner rejected it, because a way against the constitution is no option at all.
- rejected: an agent proposed that a way plainly better by the data be taken with no question and no line; the owner rejected it, because whatever a serious experiment, such as a performance test, showed better enters, and a look in a browser is none.

### decision a-decision-is-what-holds-and-the-ways-not-taken
A decision is its first line of what holds now and one or more lines under it, each a way weighed and not taken: rejected where it was proposed, replaced where it stood on main.
- replaced: A decision as what holds and a Why line, its reason and any way turned down and why it lost, stood on main; the owner changed it, because a line must state a fact, who proposed a way and who turned it down, since a way can be invented and a fact cannot.
- rejected: an agent proposed a first line opening with Because, the want or the given before what holds; the owner rejected it, because the form must state what happened, which agents cannot invent as they can a want or a way.
- rejected: an agent proposed a third kind of line, tried, for a way tried and dropped; the owner rejected it, because rejected and replaced alone are clearer, and what was researched without being done is no tried.

### decision the-owner-settles-every-choice
In a rejected line the one who proposed a way is the owner, an agent or a reviewer agent, and in every line the one who rejected or changed it is the owner.
- rejected: an agent proposed naming an agent by its cell, as R1, and the work a way was weighed in; the owner rejected it, because an agent need not understand what a cell is, and the tree must not be tied to the flow's cells.
- rejected: an agent proposed naming as the one who settled a way an agent where the owner did not answer; the owner rejected it, because nothing merges without his yes, so whichever way he takes, he decided it.
- rejected: an agent proposed a line only where the owner's answer chose between the two ways it names; the owner rejected it, because most of his answers are given in batches.

### decision a-line-names-no-moment
A line tells only what happened, naming no work, item or date, which the file's history keeps, and a reason resting on data ends with the repository file that keeps it.
- rejected: an agent proposed dropping the rule that an entry be read without its source, which keeps out words bound to their moment; the owner rejected it, because such words and cell numbers went void at each renumbering, once a great problem.

### decision a-choice-brings-its-change-of-the-tree
A choice is put with the change of the tree it brings, in substance, and the tree takes it as the answer settles it, rewriting what it makes untrue and deleting what is gone.
- replaced: Only reviewed text put to the owner, and what he confirms written word for word, stood on main; the owner changed it, because a choice need carry the change of the tree only in substance, a later check holding its content to it.
- replaced: A tree that only grew, nothing deleted and a dropped feature a decision of its own, stood on main; the owner changed it, because overturned decisions recalled beside the ones in force only mislead.
- rejected: the owner proposed keeping overturned decisions in an archive beside the tree; the owner rejected it, because agents would still read them in.

### decision the-tree-keeps-no-log
The tree keeps no log of its changes: what changed when, and in which work, is in the history of its files.
- replaced: A log line with the owner's approval for each change of the tree, checked before a merge, stood on main; the owner changed it, because the log was kept so that an agent could not make up his approval, and in the flow whatever merges was ruled.
- replaced: A log of the tree's changes, each with what changed, why and the ruling, stood on main; the owner changed it, because every part of it has another home: the change in the diff, the why in the entry, the ruling in the pull request.

### decision comments-are-not-the-code
Comments are not the code, and where a comment and an entry differ, the entry holds.
- replaced: A comment counted as part of what the code shows, a reason of one place written as its comment and not in the tree, stood on main; the owner changed it, because a comment can say anything, so no rule would stand on it.
- rejected: an agent proposed rules telling what goes into a comment from what goes into the tree, one place being one file; the owner rejected it, because the README is not about comments, which may say anything so long as they do not go against the tree.

### decision the-readme-holds-the-standards-alone
The README holds the tree's standards alone: no example, nothing of one project, and no step of a flow by which a change is asked, written or reviewed.
- replaced: A decision's plan saying it, its change writing it and its review holding it stood on main; the owner changed it, because the README sets the tree's standards and the flow how they are kept: here it is a question, elsewhere it need not be.
- rejected: an agent proposed naming examples in the rules, and the owner's rulings on border cases as precedents; the owner rejected it, because the tree is not built for this project alone, and for another project every example fails.

### decision the-trees-rules-are-its-readme
The tree's rules are its README, `design/README.md`, which AGENTS.md points to for the agents that work in the repository.
- replaced: The tree's rules as a skill in a submodule of each project stood on main; the owner changed it, because a skill is not always active, and the tree is needed most of the time.
- rejected: the owner proposed giving the tree's rules only as a rule of the flow, AGENTS.md silent on the tree; the owner rejected it, because agents that work in the repository outside the flow read AGENTS.md and found no rules of the tree.
- rejected: an agent proposed writing the tree's own rules as entries at the top of the tree; the owner rejected it, because the tree is about the project, and rules about the tree are not in it.

### decision limits-stand-only-where-the-check-holds-them
The README sets a limit on an entry only where its form or the check holds it: a first line of at most 200 characters, every other line of at most 300.
- replaced: Rules and limits in the README's prose, as no identifier from the code and a reason resting on judgment saying so, stood on main; the owner changed it, because a limit in the README is of no use unless a machine checks it, agents copying the entries there are.

### decision the-check-is-a-python-script-beside-the-tree
`design/lint.py`, a Python script beside the tree, checks its files' forms by pattern; CI's lint job runs it, and it runs by hand with no arguments from any directory.
- rejected: an agent proposed the checker as a Rust crate sharing one parser with the engine and the page; the owner rejected it, because Python needs no compiling and no toolchain, and runs at once on so small a check.
- replaced: The tree's forms held only by its README and the reviews, with no check, stood on main; the owner changed it, because a limit holds only under a mechanical check, and CI costs much, so an agent checks the tree before it.

### decision the-tree-is-written-in-english
The tree is written in English, as AGENTS.md has everything in the repository written.
- replaced: The tree written in the owner's language, its technical words in English, stood on main; the owner changed it, because the tree read better to him in English.
