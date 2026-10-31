Fact browser-reaches-loopback: A page on another site can reach a server on the loopback address through the owner's browser, by rebinding a domain name to the loopback address or by sending a cross-site request.

Fact browser-sse-retry: A browser gives up on an event stream answered with an error, as a proxy in front of `wf-agents run` answers while it restarts, and not on one cut off.

Fact checkout-branch: A project's checkout stands on whatever branch its owner left it.

Fact effort-low-no-thinking: The effort `low` does not think.

Fact gh-needs-repository: gh in a directory that is no repository knows no repository.

Fact git-config-lock-fails: With the lock file of a repository's configuration held, git does not wait for it: `git config x.y z` fails at once, exiting 255, and `git branch -D` deletes the branch but keeps its section in the configuration, with only a warning and exit 0 (git 2.56.0).

Fact git-worktrees-share: Every worktree of a repository shares the repository's common git directory: a fetch in any of them moves the same remote-tracking branches, and a worktree added on a new branch from a remote-tracking branch writes the branch's tracking into the shared configuration.

Fact global-round-numbers: With one sequence of round numbers across all works, a work's rounds jump: work 1's 102 rounds are spread over 1 to 240.

Fact one-name-confusion: With one name for a design card and an ask of the owner, agents put a question on a design card and a decision in an ask.

Fact page-writes-own-connection: The owner's page writes the database from a connection of its own for each request, beside the engine's.

Fact session-read-whole: A session is read again whole at every call, which grows the cost of each call with the work.

Fact setting-takes-effect-now: The owner changes a setting for it to take effect now.

Fact split-endings-leaked: With each way of ending a work doing part of the ending, a cancelled item's work keeps its sessions, its worktree and an open pull request, and a work that ends leaves its question in the architect's queue, which the architect answers to no one.

Fact sqlite-hooks-per-connection: SQLite's update, commit and rollback hooks on a connection are told of that connection's changes only: each row it changes, each transaction it commits and each it rolls back.

Fact sqlite-immediate-write-lock: An immediate SQLite transaction takes the database's write lock before it reads, so connections write one database one at a time, and a number one reads and writes in such a transaction is never taken twice.

Fact sqlite-wal-reads: SQLite in write-ahead-log mode lets reads run beside a write.

Fact tool-call-either: To the model, handing in `tools` and calling a tool in the middle of a round are each one tool call.
