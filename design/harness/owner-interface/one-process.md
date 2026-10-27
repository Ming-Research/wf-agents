Decision: `wf-agents run` hosts the engine, the HTTP interface and the page in one process, instead of a separate process that serves the page, because an owner's action such as answering a card must wake the engine at once instead of waiting for the engine to poll the database, live events come straight from the engine, the engine's own state (whether it runs, what it waits for, the slots in use) is in its memory and needs no table to report it, and there is one process to start and to upgrade.

Decision: The HTTP server runs on its own threads inside that process, with its own database connections, while the engine's loop stays as it is, because a page request must never hold up a step of the engine, and SQLite in write-ahead-log mode lets the server's reads run beside the engine's writes.

Decision: The built page is embedded in the `wf-agents` binary and served by `run`, instead of files installed beside the binary, because the page and the interface it calls then always come from the same build, and installing or upgrading stays one binary.

Decision: `run` takes a startup flag with which it starts dispatching nothing (no round, no tool, no outbox action) while the interface and the page stay up, because looking at the page against a copy of the data must never run agents.

Rejected:
- A separate process that serves the page and reads the database: rejected because an owner's action would reach the engine only when the engine next polls the database, and the engine's state would need a table the engine keeps writing for the other process to read.
