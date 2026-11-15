# What rounds cost

How the engine judges what the agents' rounds, the acts' rounds, the calls of tools and the works take, and tells a
round that missed its session's prompt cache.

### decision a-line-comes-from-the-history-of-its-kind
Every line a measured quantity is judged by is `2 × largest − median` of the history of its kind, in every work of the flow: an agent's round against its cell's rounds where at least `history.samples` of them tell it, else the flow's agents' rounds; a round of an act against its cell's rounds alone; a call of a tool against the calls of its tool; with fewer, nothing is judged. What is judged is no part of its own history, nor is a round that went past the lines of a signal that stops a round, a round of an act that handed nothing in, a call withdrawn before it had its result, or a round of a session replaced for its size.
Why: A fixed number cannot tell a fault from a case that is legitimately large, and was set by hand (principle `judged-by-own-history`). A value in its own history stands within the line it makes, and an anomaly let into the history raises the line for good; an act that failed, or a call withdrawn with its run or its work, was cut short by that, not by its work. With 19 values, a new value of the same kind is past all of them one time in twenty. An act or a tool has no wider kind in a flow than its own: the check before the merge waits for the project's CI, which neither the flow's other acts nor its agents' rounds do, so a line of theirs would say nothing of it (a judgment, not measured).

### decision judged-by-the-history-as-it-began
What a round cost and how long it took, and how long a call of a tool took, are read against the lines their history gave as each began: of the flow's other works, what had ended by then, and of its own work, what ended before it in the work's order.
Why: Those are the lines the run guard judged the round by as it ran. Judged by the history as it stands when read, a round would be measured against rounds that came after it, and what it was found to be would change as the history grew.

### decision the-guard-stops-on-growth-alone
The run guard stops a round only on growth of its context or a jump of its first prompt, each past two lines at once; what a round costs and how long it takes are read against their lines from the records and stop nothing. A round the owner let on after the guard stopped its run is judged and not stopped, rounds that failed otherwise between passed over.
Why: A round that is long and usual costs much and takes long as one running away does, while its context rises for each call no faster than usual; growth alone, or a large message alone, are both usual, so each signal takes two conditions. A guard that stopped the round the owner let on would leave the owner no way past it, and a failure in passing between does not take back what the owner decided.

### decision a-work-is-judged-by-the-flows-other-works
As each round ends, a work whose spend or time in rounds, counted from where it was last let on after the engine paused it, is past the line the flow's other works give waits for the owner; the work's budget, `agents.work_budget_usd`, stays beside it.
Why: The other works, finished or not, are the history of a work's kind; counted from the last letting on, a work the owner let on goes until it passes the line again rather than pausing at once. The budget is the owner's own cap, which the owner chose to keep.

### decision a-miss-is-told-from-the-records
A round missed its session's cache where it went on in its session within `sessions.cache_minutes` of the session's last call and its first call read less than half of what the session held; it is expected where something before the session's messages (model, effort, program, system prompt, schema, tools) differs from the call before, and a fault of wf-agents otherwise. It is worked out from the records whenever it is read, and not stored.
Why: What a round wrote to the cache is no measure: it also holds what the session gained since, the last answer and the round's own message, which after a round of tools is large. Each part is recorded with each try, so a miss is told whenever it is asked (principle `state-from-records`).

### decision a-round-without-a-result-is-priced-from-its-calls
A round the run guard stopped, and one whose program ended of itself with no result, as one that crashed, took what its calls to the model cost at their models' prices, as its stream told them, and its record says that what it took is an estimate.
Why: A program stopped or crashed prints no result to say what the round cost, while the round the guard stops is the costliest kind it sees: recorded as nothing, it would read as no cost and stay out of the budget (principle `nothing-passes-for-normal`). Marked, the estimate is not taken for what the program said.

### decision asking-is-judged-by-its-history
How many times in a row a cell asks, for calls of tools or for tests in a record, before a run of it hands in its work,
is a measured quantity, its line drawn from the rows of asking in every work of the flow as every line is (decision
a-line-comes-from-the-history-of-its-kind): the cell's own rows where at least `history.samples` of them tell it, else
every cell's of the flow; the row a cell is in now is no part of it. A return that asks and makes its cell's row longer
than the line pauses the work for the owner, saying the row, the line and what gave it; with too few rows nothing is
judged, and the work's budget still holds.
Why: The counts the walk held, of the calls of a run and of runs in a row that only asked, were set by hand and no work
showed what they should be (principle `judged-by-own-history`); a cell that asks for test runs again and again as it
should and one that runs away differ only against what such cells do. A count begun anew by a commit was turned down
with them: a cell that commits between two asks still asks again. Kept beside the walk, the line leaves the walk to
what the records of a work alone say.
