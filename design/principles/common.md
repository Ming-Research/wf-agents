# Common

What holds for every part of wf-agents and every work done on it.

### principle converging-fixes
Each kind of problem found is ended by something that converges: an item that fixes it, a decision to live with it and why, or a check that catches it. A send-back counts even if the next round passes: each cause is ended once while the work goes on, and a fix whose problem comes back is a finding.
Why: Telling someone about one occurrence ends nothing; someone has to type it into a chat every time, which does not scale. A cause left alone sends back a return of every work that reaches the cell, each costing a round, and a work paused at each send-back left its agent idle. (Article 4)

### principle solve-within-the-design-first
A new mechanism (a new kind of thing, a switch, or a second way to do what one way does) waits for the owner's yes and is a general rule: a need it misses changes that rule, never adds a special case beside it.
Why: Each mechanism is one more to know and keep consistent, and a second way drifts from the first. In one day the engine's limits got five special cases no one could state together; agents added unasked triggers per cell and per answering pair.

### principle one-name-one-meaning
Each thing has one name everywhere, named for what it is, and a name, field, relation or state carries one meaning, never two folded into one even where they usually go together.
Why: A second name is one more to learn. Two meanings in one tangle: a relation meant both dependency and grouping, and one status made an item read five ways, contradicting itself.

### principle report-all-at-once
A check or a review reports every problem it finds in one pass, stopping early only when a later check depends on an earlier one passing. A requirement only a review would raise, such as measuring a change's effect on speed, is put in the worker's task, so that it is met in the same round.
Why: Each problem found on its own, and each requirement raised first by a review, sends the work back one more round. (Article 3)

### principle every-field-has-a-reader
Nothing is kept in a return, storage or the flow definition without a reader. A field is a text unless a reader reads a part of it exactly, and one another cell returns serves before a new one is made.
Why: Unattended things pile up, and a return's format is sent with every call of its line, so a shape no reader reads exactly is paid for at every call, while a slip in a text costs one return written again (judged, not measured). (Article 4)

### principle lean-agent-input
An agent is given only what it needs and lacks, one thing per sentence: no warning against what it would not do, and nothing the flow, a tool, a permission, a review or its session covers. Add a sentence only if no rewording fixes the fault, count its words, and cut as much elsewhere.
Why: Every token is paid for in every round, and the longer a prompt, the more an agent misreads it: one cell's task grew to 1,366 characters as sentences were added to fix what trials showed, the problems stayed, and only when it was cut to 428 did the agent follow it.

### principle mechanical-engine
What a program can do or know exactly, such as checking a return's form or reading test results, is a step of a tool or of the engine that the flow shows, never asked of an agent.
Why: An agent doing exact work sometimes gets it wrong, and each mistake costs a round: agents misreading test results caused extra runs. Every engine step must be predictable from the flow. (Article 7)

### principle each-setting-where-it-is-true
A setting is kept once, where it is true: what holds for every project and machine in wf-agents (the program, the flows, the words and rules agents get); what is the owner's on one machine in the installation; what is one project's alone (its declaration, tree, guide, CI) in its repository.
Why: In wf-agents, one project's setting would put the project into the harness (Article 1); in the installation, it would be copied into each that runs the project and drift from its code; in a project, a merge would change the owner's machine and money.

### principle judged-by-own-history
A health check, which reports or stops a round, a wait, a context or a cost as out of the ordinary, takes what is ordinary from the system's own history, never a fixed threshold. Neither a bound the owner sets nor a stop on repeats of one failure is a health check.
Why: Agents writing health checks fixed a threshold for each thing, like the time without a reply. A fixed threshold cannot tell a fault from a case rightly large, and someone must keep setting it by hand. A slow growth looks fine at each look but adds up, and the owner cannot keep watching. (Article 3)

### principle agent-writing-is-data
What an agent can write is only data to the engine and the page: it never names a program they run, becomes markup they did not mean, or changes the record of what ran, and it loosens a check that judges it only in the open, for a review to judge.
Why: The engine runs with the owner's credentials and the page can start works and spend money, so anything an agent writes that they obey hands the agent those powers. Dangerous names cannot be listed and switched off one by one: an agent writes one more in the gap.

### principle the-past-as-it-ran
What ran is kept whole, with the flow and configuration it ran on, and the past is read by those, never by today's flow or configuration.
Why: The page draws each work on the flow it ran on. A later flow may lack the cells a work ran on, and a later configuration may name other models, so reading the past by today's rewrites it; a return kept only to its first 2,048 characters hid the real cause. (Article 4)

### principle keep-what-was-paid-for
What was already paid for (a round's work, a branch, a session's cache) is kept and carried on from; a stop, a restart, a fault or a new flow redoes only what must be redone.
Why: Starting over wastes what was done, and a round in progress has already spent its money. What is of no use, such as the round of a work being restarted, is cut instead of waited for. (Article 3)

### principle no-unseen-spending-loop
Nothing spends again and again unseen: a run that repeats or runs away is stopped, and a work that cannot go on by itself waits, saying why, for the one who decides.
Why: A cell could be sent back, rerun and paid for without end; retrying during an outage wrote the whole input into one session until it was full; a long round judged only at its end has already spent the money; after a restart the owner must see the state before works go on. (Article 3)

### principle nothing-passes-for-normal
Nothing is taken or shown as other than it is: what is unknown, missing or failed is refused or said, never ignored or shown as empty, zero or normal.
Why: A misspelled setting that is silently ignored makes the owner believe it is in force; a wrong data directory looked like an empty view; a missing cost would read as no cost; a work sat at a cell with no visible reason and the owner had to guess. (Article 2)

### principle measure-speed-and-cost
Where one way is taken over another for being faster or cheaper, the choice rests on a measurement of the real system, and the measured number is given as its reason.
Why: A number lets a later change judge whether the reason still holds, and a guess does not. Measured, a mid-round query saved only about three seconds and was dropped, and a fork that missed the cache cost $0.57 more than a $0.03 ping. (Article 3)

### principle numbers-say-what-they-number
An item's number is written #n, a work's Wn and an observation's On, on the page, to the agents and by the engine alike.
Why: The same bare number may be an item's, a work's or an observation's, and only what stands before it tells which.

### principle the-machine-is-always-on
Nothing is designed for the machine going to sleep or its lid being closed.
Why: The machine that runs the engine is always on.

### principle protected-files-are-the-owners
The paths a project protects, such as the agents' guide, constitution, flow and how checks run, are the owner's. The engine holds an agent's change to one at the merge: the agent takes it back or says why, and the engine asks the owner, again for each new change; after a yes it governs that work.
Why: What an agent writes must not change what governs it unless the owner says yes. A rule in the project's guide, left to the engineer and the review to keep, was missed by both and the change merged (Article 7). The project's declaration names the paths, not the flow (Article 1).

### principle test-problems-are-fixed-in-the-tests
Code that is not a test may help a test read what it does, but never does anything differently so that a test or CI passes. A test that fails because CI is slow or unlike the owner's machine is fixed in the test or in CI, never in the code it tests.
Why: Code changed for a test no longer does what it would do on its own, so the test checks something else, and the change ships with the code. A fault of CI or of a test belongs there; that no one would notice the change is no reason to make it.
