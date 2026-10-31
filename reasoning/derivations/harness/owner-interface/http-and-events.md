Decision http-api: The interface is HTTP with JSON under `/api/v1`, GET for reads and POST for the owner's actions, with one server-sent-events stream at `/api/v1/events` for live changes, because any client can use it with plain requests, and live changes only flow from wf-agents to the page, which a one-way event stream carries.

Rests on: C:design-on-merit, D:page-over-interface

Decision event-bus: Events come from a bus inside the process that the engine and the owner's operations publish to; an event names what changed (a round started or ended, a transition taken, a return sent back, a work paused, resumed or ended, a design card handed in, ruled or replaced, an ask of the owner put or answered, a message told to the owner or marked read, an item or the queue changed, the engine's state changed), and the client loads a snapshot and refetches only what an event names, because the page then stays current without reading everything again, and an event never has to carry the state it points to.

Rests on: C:efficient, C:design-on-merit, D:http-api

Decision event-cursor: Event ids are a cursor: a client that reconnects with the last id it saw is given every event it missed, or is told to load a new snapshot when those events are no longer kept, because a page that silently misses an event shows a state that is no longer true.

Rests on: C:information, D:event-bus

Decision event-reconnect: The page opens the stream again itself, a few seconds later and with the last id it saw (`after`), when the browser gives up on it, which it does on a stream answered with an error (as a proxy in front of `wf-agents run` answers while it restarts) and not on one cut off, because a page left without its stream showed nothing new until the owner reloaded it.

Rests on: C:information, F:browser-sse-retry, D:event-cursor

Decision generated-types: The interface's types are defined once, in Rust with serde, and the TypeScript types the page uses, the compiled flow's types included, are generated from them, and CI fails when the generated file is stale, instead of the types written by hand on both sides, because two definitions drift, the same rule that makes `db/schema.sql` the only definition of the database (D:schema-sql).

Rests on: F:copies-drift, D:schema-sql
