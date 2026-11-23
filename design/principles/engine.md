# The engine

How the engine that carries out the flow is built, and what it does for the agents.

### principle decide-purely
A function of a work's flow, configuration and records alone decides its next step, and what a changed flow would do, reading no database, clock or network and changing nothing. Its reason is data, why a cell runs or what the work awaits from whom, which the page and command line only put in words.
Why: A pure function can be tested on every case and rerun on any history: a new rule was tried on 23 recorded works, no agent run. Both defects a review confirmed were in next-step code reading and writing the database; scripted trials missed both. A wait naming nobody gets no answer. (Articles 2 and 7)

### principle state-from-records
A work's state is what its records and pushed commits say: each run, what it handed in and the versions it was built on, kept whole and only added to. What can be worked out from them, such as where a work stands or what it waits for, is not stored beside them unless measured too slow to work out.
Why: A stored position, a cell and its callers, had to be read rightly by every rule: one reading the cell alone sent a work back forever once its cell asked for a tool. Worked out from records, a position has one reading, and append-only records show any past moment, to trace a fault or test a rule.

### principle outside-behind-interfaces
Everything the engine reaches outside itself (running an agent, a tool, the clock, a notice to a person, storage) is reached through an interface with a real implementation and one for tests.
Why: The engine, its stops at any step included, can then be run thousands of times in memory with no agent paid for; and where the agents run, or which command line runs them, can change without the engine changing. (Article 1)

### principle effects-known-as-done
Every effect the engine causes is recorded under a key of what it is about and the version it was for, so that it is recognised as done and not done again. An agent's round cut off goes on from its run's record. The engine may stop at any step and goes on from the records.
Why: An engine that must not stop between two steps needs every such pair found and guarded, and a pair missed would repeat a notice, a branch or a merge. (principles keep-what-was-paid-for and no-unseen-spending-loop)

### principle dependencies-one-way
What decides depends on nothing that acts or stores, and every part of the code depends only on parts beneath it; the build refuses a dependency the other way.
Why: The engine's deciding, acting and storing grew in one part, so that the rule for the next step could not be run without the database and the rest of the engine.

### principle records-are-formed-for-every-use
What is recorded is given a form for every use from the start, not only for the use that first writes it.
Why: Records outlive the code that wrote them, and changing their form means rewriting the past.

### principle agents-get-only-what-the-flow-gives
The engine starts an agent with a configuration directory of its own, which loads none of the owner's own tools, memory or settings: what an agent sees and may use is decided by the flow and the project's declaration alone.
Why: Other tools take up context for nothing and can be written to, and what an agent reads by itself the engine cannot carry to a later session.

### principle refuse-as-little-as-must-be
Every agent is refused only heavy work on the host's processor, such as building and testing, and writing beyond its worktree, such as pushing, besides what a read-only line is refused; the engine runs for it only what its own command line cannot, such as what needs credentials.
Why: The worktree is the agent's own, and what it does there changes nobody else's work, while what it could reach beyond it the engine's own guards hold. Each command refused or handed to the engine costs a round more, and what the engine runs for an agent needs limits of its own.

### principle stored-content-is-kept-small
What is stored is kept under control: no large content is stored more than once.
Why: Without that the size of the store grows very fast.

### principle a-look-brings-back-little
What the assistant reads of the system is as small as it can be at each look without losing what matters: every command's output has a bound that a test holds, says what it left out and how to get it, and is never raw JSON.
Why: The assistant's context must stay on what it discusses with the owner: a look that brings back more than is needed fills it (Article 3), and one that leaves out what matters leaves it to guess (Article 2).

### principle engine-knows-no-cell
Neither the engine's code nor the page's names a particular cell or a particular block: each carries out, and draws, whatever the flow's definition holds.
Why: What is in the definition the owner can read on the chart and the compiler can check before anything runs (Article 7).

### principle no-tool-opens-a-browser
No tool an agent is given opens a page in a browser or takes a picture of one; searching the web and fetching its pages are another matter.
Why: The project's pages are tested with a browser in CI, so an agent needs no browser on its own machine.

### principle agents-run-apart-from-the-engine
What runs the agents is apart from what decides which cell runs, and may be on another machine: an agent's program never reaches the engine's records, files or machine, and what it needs from the engine it asks for in its return, answered in the next round.
Why: The agents are to run wherever there is room for them, and an agent that read the engine's database by a command of its own would work only beside the engine, by a way the engine neither records nor guards. Asked in the return, a request works wherever the agent runs and is recorded with the round.
