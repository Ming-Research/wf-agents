//! The owner's interface, apart from how it is served: the types it hands out, defined here once (the page's
//! TypeScript types are generated from them by [`typescript`]), the reads that fill them from the database, the bus
//! that carries what changed to the page as it happens, and what the owner asks of the running engine.
//!
//! The interface reads the database through its own read-only connections, beside the engine's writes. The engine's
//! connection tells [`events`] the rows each committed write touched; they become the events the bus hands to every
//! open stream, and an event only names what to read again.

mod bus;
mod control;
mod events;
mod read;
mod types;
mod typescript;

pub use bus::{Bus, Envelope};
pub use control::{Asked, Control, Next};
pub use events::events;
pub use read::{Error, Flows, compiled_flow, file, flows, item, items, round, work, works};
pub use types::*;
pub use typescript::typescript;
