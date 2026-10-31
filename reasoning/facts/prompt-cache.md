Fact cache-hour: A call to a session more than an hour after its last call writes the whole session to the prompt cache again.

Fact cache-per-account: An account's prompt cache holds nothing of a session another account ran.

Fact cache-per-model: A model's prompt cache holds nothing of a session another model ran.

Fact cache-prefix: The prompt cache reads a prefix of the request, tools, system and then messages, and reads the system block whole or not at all.

Fact cache-read-price: A read from the prompt cache costs 0.025 of an input token's price on Fable 5.1, 0.05 on Opus 5.5 and Sonnet 5.5, and 0.1 on Haiku 5.5.

Fact cache-same-prompt: The prompt cache is read only when everything that shapes the prompt (the command line, the model, the system prompt, the tools and the structured-output schema) is the same.

Fact cache-shared: The prompt cache is shared by the sessions of one account and model for an hour.

Fact cache-write-holds-gain: What a round writes to the prompt cache is also what its session gained since the round before: the last answer of that round and the round's own message.

Fact cache-write-reads: Writing a session to the prompt cache for an hour costs as much as forty reads of it on Opus 5.5 and Sonnet 5.5, twenty on Haiku 5.5 and eighty on Fable 5.1.

Fact cell-cli-cache-loss: A cell whose command line differs from the rest of its line loses the line's prompt cache, as the effort `low` mixed in did once and a schema that required a field all the forms of one cell held did once.

Fact effort-keeps-cache: A changed reasoning effort keeps the prompt cache while thinking stays on.

Fact rethink-cache-miss: The architect's rethink after a batch, given a schema that required `progress` while the other rounds of its line were given one that did not, reads nothing of its session from the cache, writing 212,661, 236,140 and 138,702 tokens again, about $0.55 to $0.95 at Sonnet at the end of every batch.

Fact thinking-toggle-cache: Turning thinking on or off between rounds of a session may leave nothing of its prompt cache, while rounds between `medium`, `high` and `xhigh` keep reading it.

Fact worktrees-sessions-caches: A work's worktree and sessions are caches that the database and the branch on GitHub can make again.
