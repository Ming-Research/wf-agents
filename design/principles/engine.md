### principle decide-purely
What decides what a work does next is a function of the flow and the configuration the work runs on and of its records alone: it reads no database, clock or network, and changes nothing. Its answer carries its reason as data, which cell runs and because of what, or what the work waits for and who can give it; the page and the command line show that reason in words for their reader and work out none of their own. The same function tells what a changed flow would do to each work.
Why: A function of its inputs alone can be tested on every case and run again on any recorded history: a new rule for the next step was checked against each step of twenty-three recorded works without running an agent. The two defects a review confirmed in the engine were both in the code that chose the next step, which read and wrote the database as it chose, and the scripted trials had met neither. A wait that names nobody is answered by nobody. (Articles 2 and 7)

### principle state-from-records
The state of a work is what its records and its pushed commits say: each run, what it handed in, and the versions of what it was built on, kept whole and only added to. What can be worked out from them is not stored beside them, such as where a work stands or what it waits for, unless a measurement shows that working it out is too slow.
Why: Where a work stood was stored as a cell and the cells that had called it, and each rule had to read that position rightly: one that read the cell alone sent a work back without end as soon as its cell asked for a tool. A position worked out from the records has one reading. Records only added to can be read as of any moment, which is how a fault is traced and a rule checked against the past. (principles one-thing-one-place and the-past-as-it-ran)

### principle outside-behind-interfaces
Everything the engine reaches outside itself (running an agent, a tool, the clock, a notice to a person, storage) is reached through an interface with a real implementation and one for tests.
Why: The engine, its stops at any step included, can then be run thousands of times in memory with no agent paid for; and where the agents run, or which command line runs them, can change without the engine changing. (Article 1)

### principle effects-known-as-done
Every effect the engine causes is recorded under a key of what it is about and the version it was for, so that it is recognised as done and not done again. An agent's round cut off goes on from its run's record. The engine may stop at any step and goes on from the records.
Why: An engine that must not stop between two steps needs every such pair found and guarded, and a pair missed would repeat a notice, a branch or a merge. (principles keep-what-was-paid-for and no-unseen-spending-loop)

### principle simulated-histories
The engine is kept right by simulated histories run at every change: agents whose returns are drawn from all the flow allows, an owner who confirms, refuses, comments, pauses and cancels at any moment, an engine stopped at any step and a flow changed midway. After every step the properties the engine must always keep are checked, each of them written as a decision of the tree. A fault found any other way is added as a history that shows it.
Why: An example tests the way its writer thought of. The two defects a review confirmed were in ways no scenario had been written for: a cell that asked for a tool after being sent back, and a work that went to design after it had implemented. (principle flow-tests-formal)

### principle dependencies-one-way
What decides depends on nothing that acts or stores, and every part of the code depends only on parts beneath it; the build refuses a dependency the other way.
Why: The engine's deciding, acting and storing grew in one part, so that the rule for the next step could not be run without the database and the rest of the engine.

### principle generalise-at-the-second-use
A mechanism is written for the one use it has, never for one project (Article 1), and made general when a second use is being built. What is recorded is the exception: its form is made for every use from the start.
Why: An abstraction made before its second use guesses that use, and the guess is paid for at every reading of the code and again when the real use differs. Records outlive the code that wrote them, and changing their form means rewriting the past. (Article 4)
