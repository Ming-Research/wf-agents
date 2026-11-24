# The design tree

What the design tree holds, how its entries are written and how they come about, and the check that enforces their forms.

### decision the-tree-records-what-the-code-loses
The design tree is a record, kept beside the code, of what was weighed against each choice, what was tried and why one approach won, which the code does not keep.
- replaced: A system of derivations from the constitution, with facts, Open and Assumed marks and a check of its coherence, was on main; the owner changed it, because agents can keep a memory of facts themselves, and facts contain no errors of logic.
- replaced: Keeping the owner's decisions as lines on the items they were settled in was on main; the owner changed it, because a decision on an item that was later overturned, recalled beside the decisions in force, only misleads.
- rejected: an agent proposed keeping the decisions in the engine's database, by project; the owner rejected it, because the database is separate from the code, and a directory beside the code keeps them with it.
- rejected: an agent proposed that the engine write the tree's files from the changes an agent hands in; the owner rejected it, because the tree is read freely, as code is, not through a database, and it works without the engine.

### decision the-tree-holds-three-kinds
The tree holds three kinds: the constitution, the owner's principles for everything built from now on, and the decisions about what is true of one part of the system.
- replaced: Design documents as a fourth kind in the tree, each a design that holds only as a whole and may run ahead of the code, was on main; the owner changed it, because two sentences of the README on them pointed two ways, and they were cited by paragraph as decisions are, yet passed no bar.
- rejected: an agent proposed facts as a kind of their own, listed with ids, with the principles and decisions resting on them by links; the owner rejected it, because facts stay in the items, and decisions that match the code are checked against the code.
- rejected: an agent proposed intent, norm and limit as kinds of entry; the owner rejected it, because the three blur together, with agents turning his intents into norms, and the tree is kept to categories and decisions.
- rejected: an agent proposed that every principle and decision be derived from the constitution; the owner rejected it, because that claims too much, and entries forced to derive from it make up their reasons.

### decision a-concept-holds-its-entries
Each directory is a tree of concepts that need not follow the code's layout, and an entry is written once, in the file of the lowest concept that covers everything the entry is about.
- replaced: One file of decisions for each ability or component, with a decision about two of them written in one file and pointed to from the other, was on main; the owner changed it, because a concept above holds what several share, and pointing lines went stale and were what agents missed most.
- rejected: an agent proposed laying the tree out like the code's tree; the owner rejected it, because laid out by the code, many things would sit in very strange places.
- rejected: an agent proposed one file for each decision, with its first line read by a search; the owner rejected it, because agents cannot read thousands of decisions, and a hierarchy of categories, with several decisions to a file, finds what is needed in log n.
- rejected: an agent proposed links between entries that the engine follows, and an index of the tree that it builds; the owner rejected it, because three earlier reorganisations of the tree failed from complexity and corner cases.
- replaced: A root of the tree named after the project, beside the directory of its concepts, was on main; the owner changed it, because the tree's directory is already in the project.

### decision a-file-of-the-tree-is-plain-text
A file of the tree is Markdown: its title, one line saying what its concept covers, and its entries one blank line apart, with each line of an entry on one line of the file.
- rejected: an agent proposed the tree as structured data, JSON, TOML or YAML, with a view rendered for reading; the owner rejected it, because every work reads and writes the tree, which is easier as prose, and tools rarely parse it.

### decision a-decision-says-only-what-main-does
A decision's first line says only what the code does on main, while a principle may come before the code.
- replaced: A tree ahead of the code, where any difference was code behind the tree, for an agent to fix or report, was on main; the owner changed it, because decisions that match the code are easy to keep consistent and to check against it.
- replaced: A file at the root of the repository holding the designs not yet true of main was on main; the owner changed it, because there is no pending design, and a design ahead of the code is a document, which may run ahead of it.
- rejected: an agent proposed a line on each entry saying what the code does not do yet, and which item will do it; the owner rejected it, because a status kept on entries drifts from reality.
- rejected: an agent proposed a rule that the whole tree hold only what the code on main does; the owner rejected it, because principles give direction, and so run ahead of the code.

### decision a-principle-is-the-owners-rule-for-a-kind
A principle is a rule for everything of one kind, including things not built yet; an agent may propose one or a change to one, and the owner decides.
- replaced: Only the owner adding or changing a principle, with an agent saying where one is missing or wrong, was on main; the owner changed it, because an agent that fully understands the principles can help keep them, or at least propose a fix.
- replaced: Admitting a principle only when two places that share no code already use it was on main; the owner changed it, because a decision explains something that exists, while a principle binds what is not built yet.
- rejected: an agent proposed as a principle a rule for one place in the code; the owner rejected it, because a rule that governs one place is too narrow to be a principle.
- rejected: an agent proposed that a principle be one that every agent working in its scope needs; the owner rejected it, because who reads a principle is a criterion for its use, not for what enters.

### decision a-principle-is-its-rule-and-its-why
A principle is its rule, in one or two sentences of at most 300 characters, and its reason after `Why:`, of at most 300 characters.
- rejected: an agent proposed writing a principle as When, Do and Why lines; the owner rejected it, because When and Do split one thing and read badly.
- rejected: an agent proposed a Rests on line naming what a principle rests on; the owner rejected it, because 26 of 28 principles had one without telling agents anything more.

### decision a-decision-comes-from-a-choice-put-to-the-owner
A decision comes only from a choice put to the owner: an approach against a decision, two or more approaches that would each work, or a change to an important mechanism no decision covers.
- replaced: Every change no decision covers to what the owner sees or does being the owner's choice was on main; the owner changed it, because agents then asked him about a look the page already had for the same meaning, against an option that did not do what the item asked.
- replaced: A decision entering when an agent that had read the code and the principles would still change it wrongly was on main; the owner changed it, because that bar, asked of agents in their prompt, failed: agents copied the entries beside them and invented.
- replaced: Decisions entering through the work that makes them true without the owner's yes, which only principles waited for, was on main; the owner changed it, because questions and the tree become one, so the tree's gate is the gate of questions.
- rejected: the owner proposed MCTS-Mem's gate, where only what a change modifies enters; the owner rejected it, because it kept out the choices weighed in research.
- rejected: the owner proposed Whitefoot's check that each decision say "because" or "instead of"; the owner rejected it, because it let in too much and made agents invent ways to write.
- rejected: an agent proposed a second line, "Not knowing this, an agent would", telling what an agent would do wrong without the entry; the owner rejected it, because it is not a good gate.
- rejected: the owner proposed that a question that stops the work enter, and one that does not stop it enter only when he takes an approach other than the one recommended; the owner rejected it, because whichever approach he takes, the question and the tree are one.
- replaced: Leaving out of the tree what the code shows, such as what a thing does, its names and a property a test guards, was on main; the owner changed it, because seeing what the code does is not understanding why it does it.
- rejected: an agent proposed that what a check enforces be taken out of the tree and left to the check; the owner rejected it, because a check is code too, which an agent not knowing what it checks may change.
- rejected: an agent proposed entering designs once the code made them true; the owner rejected it, because being built is no reason to enter, and each must meet the bar.
- rejected: an agent proposed entries for things settled with no other approach weighed, such as the look of a label; the owner rejected it, because trifles and boring decisions do not enter the tree.
- rejected: an agent proposed a sentence, separate from the kinds, on what is not written; the owner rejected it, because whether a thing enters is said in the definitions of a principle and a decision, and nowhere else.

### decision a-choice-weighs-ways-that-would-each-do
Two approaches make a choice only when each would work and each was properly researched or tried; an approach that is simply better is taken without asking, unless an experiment showed it.
- replaced: A card of at least three viable approaches, never doing against not doing, was on main; the owner changed it, because often only two approaches are real, and forcing a third makes up options nobody would pick.
- rejected: an agent proposed keeping a list of the approaches looked at and found not to be options; the owner rejected it, because an approach that is no option has no value, and one properly explored is an option turned down.
- rejected: an agent proposed that an approach with pros and cons of its own makes a choice; the owner rejected it, because a choice is a trade-off between the approaches, and an approach beaten in every respect makes none.
- rejected: a reviewer agent proposed that a choice is real only when a changed premise would turn a rejected approach into the chosen one; the owner rejected it, because a choice weighs the approaches' pros and cons whether or not any premise changes.
- rejected: an agent proposed counting an approach that breaks an article of the constitution as an option with a cost; the owner rejected it, because an approach against the constitution is no option at all.
- rejected: an agent proposed that an approach the data shows to be simply better be taken with no question and no line; the owner rejected it, because whatever a serious experiment, such as a performance test, showed to be better enters, and looking in a browser is no experiment.

### decision a-decision-is-what-holds-and-the-ways-not-taken
A decision is a first line saying what is true now and one or more lines under it, each an approach considered and not taken: rejected if it was proposed, replaced if it was on main.
- replaced: A decision as what is true and a Why line holding its reason and any approach turned down and why it lost, was on main; the owner changed it, because a line must state a fact, who proposed an approach and who turned it down, since an approach can be invented and a fact cannot.
- rejected: an agent proposed a first line opening with Because, putting the need or the given before what is true; the owner rejected it, because the form must state what happened, which agents cannot invent as they can a need or an approach.
- rejected: an agent proposed a third kind of line, tried, for an approach tried and dropped; the owner rejected it, because rejected and replaced alone are clearer, and what was researched without being done does not count as tried.

### decision the-owner-settles-every-choice
In a rejected line the one who proposed an approach is the owner, an agent or a reviewer agent, and in every line the one who rejected or changed it is the owner.
- rejected: an agent proposed naming an agent by its cell, such as R1, and the work an approach was weighed in; the owner rejected it, because an agent need not understand what a cell is, and the tree must not be tied to the flow's cells.
- rejected: an agent proposed naming an agent as the one who settled an approach when the owner did not answer; the owner rejected it, because nothing merges without his yes, so whichever approach he takes, he decided it.
- rejected: an agent proposed a line only when the owner's answer chose between the two approaches it names; the owner rejected it, because most of his answers are given in batches.

### decision a-line-names-no-moment
A line says only what happened and names no work, item or date, which the file's history keeps, and a reason that rests on data ends with the repository file that keeps the data.
- rejected: an agent proposed dropping the rule that an entry be read without its source, which keeps out words tied to their moment; the owner rejected it, because such words and cell numbers lost their meaning at each renumbering, which was once a great problem.

### decision a-choice-brings-its-change-of-the-tree
A choice is put with the change to the tree it brings, in substance, and the tree takes the change as the answer settles it, rewriting what it makes untrue and deleting what is gone.
- replaced: Putting only reviewed text to the owner, and writing what he confirms word for word, was on main; the owner changed it, because a choice needs to carry the change to the tree only in substance, and a later check holds the tree's content to it.
- replaced: A tree that only grew, deleting nothing and making a dropped feature a decision of its own, was on main; the owner changed it, because overturned decisions recalled beside the ones in force only mislead.
- rejected: the owner proposed keeping overturned decisions in an archive beside the tree; the owner rejected it, because agents would still read them in.

### decision the-tree-keeps-no-log
The tree keeps no log of its changes: what changed when, and in which work, is in the history of its files.
- replaced: A log line with the owner's approval for each change to the tree, checked before a merge, was on main; the owner changed it, because the log was kept so that an agent could not make up his approval, and in the flow the owner has ruled on whatever merges.
- replaced: A log of the tree's changes, each with what changed, why and the ruling, was on main; the owner changed it, because every part of it has another home: the change in the diff, the why in the entry, the ruling in the pull request.

### decision comments-are-not-the-code
Comments are not part of the code, and where a comment and an entry disagree, the entry is right.
- replaced: Counting a comment as part of what the code shows, so that a reason for one place was written as its comment and not in the tree, was on main; the owner changed it, because a comment can say anything, so no rule could rest on it.
- rejected: an agent proposed rules telling apart what goes into a comment and what goes into the tree, with one place being one file; the owner rejected it, because the README is not about comments, which may say anything as long as they do not go against the tree.

### decision the-readme-holds-the-standards-alone
The README holds only the tree's standards: no example, nothing specific to one project, and no step of a flow by which a change is asked for, written or reviewed.
- replaced: Steps in which a decision's plan states it, its change writes it and its review checks it was on main; the owner changed it, because the README sets the tree's standards and the flow sets how they are kept: here it is a question, elsewhere it need not be.
- rejected: an agent proposed naming examples in the rules, and the owner's rulings on border cases as precedents; the owner rejected it, because the tree is not built for this project alone, and for another project every example fails.

### decision the-trees-rules-are-its-readme
The tree's rules are its README, `design/README.md`, which AGENTS.md points to for the agents that work in the repository.
- replaced: The tree's rules as a skill in a submodule of each project was on main; the owner changed it, because a skill is not always active, and the tree is needed most of the time.
- rejected: the owner proposed giving the tree's rules only as a rule of the flow, with AGENTS.md saying nothing about the tree; the owner rejected it, because agents that work in the repository outside the flow read AGENTS.md and found no rules for the tree.
- rejected: an agent proposed writing the tree's own rules as entries at the top of the tree; the owner rejected it, because the tree is about the project, and rules about the tree are not in it.

### decision limits-stand-only-where-the-check-holds-them
The README sets a limit on an entry only where its form or the check enforces it: a first line of at most 200 characters, and every other line of at most 300.
- replaced: Rules and limits in the README's prose, such as no identifier from the code and a reason resting on judgment saying so, was on main; the owner changed it, because a limit in the README is of no use unless a machine checks it, since agents copy the entries that are there.

### decision the-check-is-a-python-script-beside-the-tree
`design/lint.py`, a Python script beside the tree, checks the forms of its files by pattern; the full CI's `tree` job and the fast CI's lint run it, and it can be run by hand from any directory.
- rejected: an agent proposed the checker as a Rust crate sharing one parser with the engine and the page; the owner rejected it, because Python needs no compiling and no toolchain, and runs at once for so small a check.
- replaced: The full CI's lint job running it was on main; the owner changed it, because a change of only the tree should run only the checks of the tree, and the lint job compiles Rust.
- replaced: The tree's forms kept only by its README and the reviews, with no check, was on main; the owner changed it, because a limit holds only under a mechanical check, and CI costs a lot, so an agent checks the tree before CI does.

### decision the-tree-is-written-in-english
The tree is written in English, as AGENTS.md says everything in the repository is.
- replaced: The tree written in the owner's language, with its technical words in English, was on main; the owner changed it, because the tree read better to him in English.
