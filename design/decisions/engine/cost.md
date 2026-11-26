# What rounds cost

How the engine judges what rounds, tool calls and works take, the size of a tool call's result included, where what went past its line is given, and how it tells that a round or a fork missed its cache.

### decision a-line-comes-from-the-history-of-its-kind
Every threshold line that a round, an act or a tool call is judged by is worked out from the history of its kind on the flow; where there are too few of them, nothing is judged.
- replaced: The number of values a history needs before it gives a line, set in the installation, was on main; the owner changed it, because it is the same on every machine.
- rejected: an agent proposed fixed limits on a round, a work and a day, and a session's limit lowered by hand; the owner rejected it, because a number set by hand that pins one point is not a good way.
- rejected: an agent proposed three fixed limits on what is sent to an agent, on one input, on a round's own content and on a whole message; the owner rejected it, because a fixed threshold misses small faults and flags large sends that are sound.

### decision the-guard-stops-on-growth-alone
The run guard stops a round only when its context grows, or its first prompt jumps, past two threshold lines at once; what a round costs and how long it takes stop nothing.
- rejected: an agent proposed cutting off a long round at a threshold line and handing its session over to a new one in the same cell; the owner rejected it, because how an agent uses its own tools is not the system's efficiency, and the system's efficiency is what needs fixing.

### decision a-letting-on-allows-what-the-installation-says
Each time the owner lets on a work the engine paused for its spending, it may spend `agents.let_on_allowance_usd` more than it had spent then; the words beside it say how much more.
- replaced: Adding the allowance to the bar the work had been paused at was on main; the owner changed it, because a round that spent more than the allowance left the work over its new bar, and a work let on was paused again at its next round's end.
- replaced: Letting a work paused for its budget run again with nothing more to spend was on main; the owner changed it, because one press then bought a single round, and the work was paused again at its end.
- replaced: Words beside a work paused for its budget that said only how much it had spent over its budget was on main; the owner changed it, because they should say how much letting it run again adds.

### decision a-miss-is-told-from-the-records
A round or a fork missed its session's cache if, while it was alive, its first call read under half of what the session held; a round's miss is expected, wf-agents' fault or of unknown cause.
- rejected: an agent proposed counting a miss in the cost figures a project's architect reads; the owner rejected it, because the project being run need not be wf-agents, and a miss is wf-agents' to fix, so it should come to him.
- replaced: Saying a fork was not read from the cache when it read under 80% of the session was on main; the owner changed it, because the same thing was then defined in two places, a round's miss and a fork's.
- rejected: an agent proposed one test for both at 80%; the owner rejected it, because more rounds would be judged as misses, and the history of works already run would be judged again.
- replaced: Telling a miss nothing explained as wf-agents' fault, whatever the records held, was on main; the owner changed it, because a fault should always be worth looking into, and a round the records do not explain shows as not known.

### decision a-compacted-session-is-no-miss
A round with a compaction recorded whose first prompt is shorter than what its session held compacted it before its first call, and is no miss.
- replaced: Counting such a round as a miss, a fault of wf-agents where nothing before the messages differed, was on main; the owner changed it, because after a compaction the cache has nothing of the session to read, and it is no fault of wf-agents.
- rejected: an agent proposed listing it as an expected miss with the cause compaction; the owner rejected it, because the session it read was replaced by design, and the list should hold only what can be found a cause for; the compaction's cost stays in the round's compactions and cost.

### decision what-went-past-its-line-is-given-in-data-only
What went past the line of its history (a round's cost or time, a call's time or result size) is given in the System data and by wf-agents system; the page draws only the lines of rounds under way.
- replaced: The page drawing none of it, rounds under way included, was on main; the owner changed it, because a round under way's figures against their lines show which round to look at (decision a-round-under-way-shows-its-figures-against-their-lines).
- rejected: an agent proposed marking a call's result size past its line on the Step and listing such calls in the System view; the owner rejected it, because the page's marks for the three kinds of line are to be designed together, so that like content looks alike.

### decision a-calls-kind-is-its-tool
A call of a tool is judged, for its time and for the size of its result, against the earlier calls of the same tool, whatever it was asked for.
- rejected: an agent proposed judging a call against the earlier calls of its tool whose requests fill the same fields; the owner rejected it, because it is a new rule that also changes how time is judged, thins each kind's history, and still misses a full result as large as the other full ones.

### decision a-cost-not-reported-is-priced-from-reports-or-not-known
A call the program reports no cost of is priced from its earlier reports of the work's calls on the model; with none, its cost is kept and shown as not known, and the work waits for the owner.
- replaced: Pricing such a call from a table of each model's list prices, by the model's name, and a model the table did not name at the dearest on it, was on main; the owner changed it, because models are upgraded and a table falls behind.
- rejected: an agent proposed adding the new models to the table; the owner rejected it, because the table would fall behind again.
