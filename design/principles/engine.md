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

### principle generalise-at-the-second-use
A mechanism is written for the one use it has, never for one project (Article 1), and made general when a second use is being built. What is recorded is the exception: its form is made for every use from the start.
Why: An abstraction made before its second use guesses that use, and the guess is paid for at every reading of the code and again when the real use differs. Records outlive the code that wrote them, and changing their form means rewriting the past. (Article 4)

### principle agents-get-only-what-the-flow-gives
The engine starts an agent with a configuration directory of its own, which loads none of the owner's own tools, memory or settings: what an agent sees is decided by the flow and the project's configuration alone.
Why: Other tools take up context for nothing and can be written to, and what an agent reads by itself the engine cannot carry to a later session.

### principle refuse-as-little-as-must-be
Every agent is refused two kinds of command: heavy work that takes the host's processor, such as building and testing, and writing beyond its worktree, such as pushing. All else is allowed, running, writing and the network included, except what a read-only line is refused besides.
Why: The worktree is the agent's own, and what it does there changes nobody else's work, while what it could reach beyond it the engine's own guards hold. Each command refused has to go round by the engine, a round more each time.

### principle the-engine-runs-only-what-needs-it
The engine runs for an agent only what the agent's own command line cannot do, such as what needs credentials: pushing, and reading the host of the repository.
Why: Handed to the engine, each request costs a round more, and what the engine runs for an agent needs a whole set of limits of its own.

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
What runs the agents (starting an agent's program, keeping its sessions, ending it) is apart from what decides which cell runs, and the two may be on different machines: nothing is designed on the ground that an agent's program can reach the engine's records, its files or its machine.
Why: The agents are to run wherever there is room for them, and an agent that read the engine's database with a command of its own would work only beside the engine, and would be a second way to the records that the engine neither records nor guards.

### principle what-the-engine-holds-is-asked-in-the-return
What an agent needs from the engine, such as its records or a check, it asks for in its return, and the engine gives the result to the round that follows; no tool or command of the agent's own program reaches the engine.
Why: To the model a tool call is the same whether its program or the engine answers it, a return asking for one only ending the round with that intent; asked in the return, a tool works wherever the agent runs, is recorded with what the round handed in, and is declared on its cell on the chart.

### principle the-records-are-found-in-one-query
Whatever an agent is told by name, each version of a work's data above all, it finds in the engine's records by one read-only query of the work, the name and the version, the store described for agents in a short text made from the schema's own comments.
Why: Agents write their own queries (principle what-the-engine-holds-is-asked-in-the-return), and one written from a wrong guess about the store reads the wrong thing or nothing, which the agent then judges by (Article 2); a description kept apart from the schema drifts from it. This rests on judgment.
