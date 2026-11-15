# What rounds cost

How the engine judges what the agents' rounds and the works take, and tells a round that missed its session's prompt
cache.

### decision a-line-comes-from-the-history-of-its-kind
Every line a measured quantity is judged by is `2 × largest − median` of the history of its kind, in every work of the flow: a round's against its cell's rounds where at least `history.samples` of them tell it, else the flow's; with fewer, nothing is judged. What is judged is no part of its own history, and neither is a round that went past the lines of a signal that stops a round, stopped or let run, nor the rounds of a session replaced for its size.
Why: A fixed number cannot tell a fault from a case that is legitimately large, and was set by hand (principle `judged-by-own-history`). A value in its own history stands within the line it makes, and an anomaly let into the history raises the line for good. With 19 values, a new value of the same kind is past all of them one time in twenty.

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
