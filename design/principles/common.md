### principle converging-fixes
A kind of problem, once found, is ended by something that converges: an item that fixes it, a decision to live with it recorded with its reason, or a check that catches it. A return sent back is such a problem even when the cell's next round passes: each cause is found once and ended so, while the work goes on; a fixed problem that comes back is a finding about that fix.
Why: Telling someone about one occurrence ends nothing; someone has to type it into a chat every time, which does not scale. A cause left alone sends back a return of every work that reaches the cell, each costing a round, and a work paused at each send-back left its agent idle. (Article 4)

### principle question-the-solution
Once a way to solve a problem is chosen, ask again, by reasoning about the problem rather than listing options, whether it is the best way and whether a better one exists.
Why: Without that reasoning, a fix solved one problem while making others, and the work did not converge.

### principle solve-within-the-design-first
Before a new mechanism is added to the system (a new kind of thing, or a new rule in how it works: a special case, a switch, a second way of doing what one way already does), a clean solution is looked for in the design as it stands: one with no case handled apart, no part going around another and no rule that holds only with exceptions (Article 6). Only where the design as it stands cannot solve the problem cleanly is a new mechanism added.
Why: Every mechanism is one more that each agent and each later design must know and keep consistent with the rest, and a second way of doing one thing drifts from the first (principle one-thing-one-place). Twice, in designing how the owner's words reach a work's cells, a new rule was proposed where data with versions and an input of each cell's own already served (the design of data and context).

### principle one-thing-one-place
Each thing (a value, a computation, a capability, a rule, a reason) is defined in one place and referred to elsewhere. It has one name everywhere, named for what it is, and a name, field, relation or state carries one meaning, never two folded into one even where they usually go together.
Why: Two copies drift apart and contradict each other, and a second name is one more thing every reader must learn: reasons once written into the flow definition were sent to agents every round and drifted from the tree's. Two meanings in one tangle: a relation that meant both dependency and grouping, and a status that made an item read five ways and contradict itself.

### principle report-all-at-once
A check or a review reports every problem it finds in one pass, stopping early only when a later check depends on an earlier one passing. A requirement only a review would raise, such as measuring a change's effect on speed, is put in the worker's task, so that it is met in the same round.
Why: Each problem found on its own, and each requirement raised first by a review, sends the work back one more round. (Article 3)

### principle every-field-has-a-reader
Nothing is kept in a return, storage or the flow definition without a reader: a cell, a page or a person who reads it. An agent is not made to fill a fixed list of questions: what does not apply is left out.
Why: Unattended things pile up, and a return's format is sent with every call of its line. A question an agent cannot answer forces it to make something up, and later steps act on it. (Article 4)

### principle lean-agent-input
An agent is given only what it needs and does not already have: one thing per sentence; no warning against what it would not do anyway; nothing the flow, a tool or a permission already decides, or a review catches as a slip; nothing a continued session already holds. A prompt is fixed by rewriting a sentence rather than adding one; what must be added is paid for by cutting as much elsewhere, so that the prompt ends no longer than before.
Why: Every token is paid for in every round. The longer a prompt, the more an agent misreads it and the more its sentences seem to contradict each other: one cell's task grew to 1,366 characters as sentences were added to fix what trials showed, the problems stayed, and only when it was cut to 428 did the agent follow it.

### principle read-code-and-tree
To understand something or change a design, an agent reads both the code and the tree's decisions about it. It reads the decisions for what was decided and why, not as orders, and follows a decision's From when it needs the data behind it; when the right solution needs a decision or a principle changed, it proposes the change.
Why: Code gives no reasons and does not say which paths were rejected; a decision is a summary that holds neither all the code does nor the data it rests on. Treating the tree as orders bends solutions to fit it.

### principle mechanical-engine
The engine only carries out the compiled flow: it takes the row a return selects and runs the actions and checks written there. Judgment belongs to agents and the owner, and what a program can do or know exactly is done by a tool or the engine, as a step the flow shows, never asked of an agent.
Why: A judgment needs someone who can give reasons and be reviewed, and every engine step must be predictable from the flow. An agent doing exact work sometimes gets it wrong, and each mistake costs a round: agents misreading test results caused extra runs. (Article 7)

### principle stateless-cells
A stage or a page that waits for something, or must remember how far it got, is not built as a small state machine that stops in several places and patches state together; the state machine itself is changed instead.
Why: Greying out a button or patching state only works around the problem; what is wrong in the state machine stays.

### principle harness-only
A project's configuration and data are in the owner's data directory, the settings projects share kept together and the installation's own rules beside them; a project's constitution, design tree and AGENTS.md are in its repository.
Why: Kept in this repository, a project's configuration would put the project into the harness (Article 1); kept in each project's repository, shared settings would be copied many times.

### principle judged-by-own-history
A measured quantity, such as cost, build time, test time or a round's context, is judged against its own past or by an exact check, never by one fixed threshold for all: the system reports a slow growth that leaves its normal range, and a guard stops a case its history does not explain. A stop on a count of the same failure is no measurement and keeps its fixed count.
Why: Each look at a slow growth shows nothing wrong, but it adds up, and the owner cannot keep watching or sending agents to look. A fixed threshold cannot tell a fault from a case that is legitimately large, and someone must keep setting it by hand. (Article 3)

### principle tech-mainstream
A technology is chosen for being mature, mainstream and fit for the project's size, without comparing every alternative.
Why: Such comparisons cost time, and SQLite and React were chosen without one; that a full comparison would not have chosen better is an inference.

### principle agent-writing-is-data
What an agent can write is only data to the engine and the page: it never names a program they run, becomes markup they did not mean, or changes the record of what ran, and it loosens a check that judges it only in the open, for a review to judge.
Why: The engine runs with the owner's credentials and the page can start works and spend money, so anything an agent writes that they obey hands the agent those powers. Dangerous names cannot be listed and switched off one by one: an agent writes one more in the gap.

### principle the-past-as-it-ran
What ran is kept whole, with the flow and configuration it ran on, and the past is read by those, never by today's flow or configuration.
Why: A work goes on from what earlier works on its item left, and the page draws each work on the flow it ran on. A later flow may lack the cells a work ran on and a later configuration may name other models, so reading the past by today's rewrites it; a return kept only to its first 2,048 characters hid the real cause. (Article 4)

### principle keep-what-was-paid-for
What was already paid for (a round's work, a branch, a session's cache) is kept and carried on from; a stop, a restart, a fault or a new flow redoes only what must be redone.
Why: Starting over wastes what was done, and a round in progress has already spent its money. What is of no use, such as the round of a work being restarted, is cut instead of waited for. (Article 3)

### principle no-unseen-spending-loop
Nothing spends again and again unseen: a run that repeats or runs away is stopped, and a work that cannot go on by itself waits, saying why, for the one who decides.
Why: A cell could be sent back, rerun and paid for without end; retrying during an outage wrote the whole input into one session until it was full; a long round judged only at its end has already spent the money; after a restart the owner must see the state before works spend again. (Article 3)

### principle nothing-passes-for-normal
Nothing is taken or shown as other than it is: what is unknown, missing or failed is refused or said, never ignored or shown as empty, zero or normal.
Why: A misspelled setting that is silently ignored makes the owner believe it is in force; a wrong data directory looked like an empty view; a missing cost would read as no cost; a work sat at a cell with no visible reason and the owner had to guess. (Article 2)

### principle measure-speed-and-cost
A choice made for speed or cost rests on a measurement of the real system, and the measured number is given as its reason.
Why: A number lets a later change judge whether the reason still holds, and a guess does not. Measured, a mid-round query saved only about three seconds and was dropped, and a fork that missed the cache cost $0.57 more than a $0.03 ping. (Article 3)

### principle owner-never-needs-the-command-line
Everything the owner needs to see or do is on the page; the command line is never the only way, and what only another program can do is shown as a command to copy.
Why: The owner does not like using a command line, and editing the configuration by hand and the database by command is the path that failed; a flow change's cost once came only from the command line. (Article 8)

### principle numbers-say-what-they-number
An item's number is written #n, a work's Wn and an observation's On, on the page, to the agents and by the engine alike.
Why: The same bare number may be an item's, a work's or an observation's, and only what stands before it tells which.

### principle the-machine-is-always-on
Nothing is designed for the machine going to sleep or its lid being closed.
Why: The machine that runs the engine is always on.

### principle protected-files-are-the-owners
The files that govern the agents are protected: the project's guide for its agents, its constitution, the definition of the flow, and what says how the project's checks run; which paths these are, the project's configuration says. The owner changes them. Where an agent's change touches one, the engine holds it: the agent takes the change back, or gives its reason, and the reason is a question to the owner. Changed in a work's branch, by the owner or with the owner's yes, such a file holds for that work from then on, as any file's new content does.
Why: What an agent can write must not change what governs it, unless the owner has said yes.
