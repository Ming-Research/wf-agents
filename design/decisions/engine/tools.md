### decision tools-use-main-as-last-fetched
The tools take main as the engine last brought it in, and never fetch main themselves.
Why: A fetch outside the lock the acts hold while they work in the repository would race their own fetches and merges; main as last brought in is never older than what the work's branch took in, so what the tools find of the branch against main comes out the same.

### decision a-query-is-refused-writes-by-the-connection
The engine runs an agent's `query` on a connection to the store that refuses every write itself: opened read-only, with `query_only` on, and with an authorizer that allows only a select, the reading of what is not hidden from agents, and functions; nothing may be attached. A query is bounded in rows, in time, in bytes (a row may not take more than the bytes a result is kept with, the first included, and one over it is refused), and in the length of any value, however it is made. A query withdrawn is interrupted.
Why: A check of the SQL's text for writes was turned down: it lists dangerous words, and an agent writes one more past the list (principle agent-writing-is-data). The time bound runs between the VM's instructions, so one call of a function over very long values is not stopped by it; the length of a value is what bounds that call's cost, whatever builds the value, where naming the functions that build large values (`zeroblob`, `randomblob`, `printf`) is again a list to write past.

### decision a-query-result-is-kept-whole-once-and-given-cut
A result of `query` is kept whole in the records, once per call, and a message gives a long one cut to the first rows, with its size and the query that reads the rest, and, where the first row alone is too long, that row with its long values shortened, so that each cut moves on.
Why: What ran is kept whole (principle the-past-as-it-ran): a query run again later may answer otherwise, since rows are added, and the agent is told what it saw. Principle stored-content-is-kept-small is met in that the copy is of what the agent asked for, once, and bounded by the bytes a result is kept with; a reference to the query alone was turned down, as it would keep no more than the query. The agent's context holds only what it can use, which is why the message is cut and the records are not.

### decision the-stores-comments-are-what-agents-are-told
What `{store}` in the words of `query` is filled with, by `carry::store`, is made from the comments of `db/schema.sql`, each table's and each column's. A table or a column whose comment says "hidden from agents" is left out of it and cannot be read by a query. The tool is off until the settings open it (`query.open`), and cannot be opened while the schema hides nothing, which is a fault at start: what agents may see of the store, and under which names, is the owner's to mark. With it off, a request is answered from the tool's file that the store is not open.
Why: A description written apart from the schema drifts from it. Marking nothing hidden would give agents the machine's paths and what they are not meant to read by default; the owner is made to choose.

### decision a-tools-result-is-kept-whole-and-given-cut
What a tool says is kept whole in the work's records, and cut only where a round is given it, at the bound its tool's words name.
Why: Cut where the host made it, the engine's records kept only what the agent was given, and what was cut could never be read again; the bound is the round's, not the result's.
