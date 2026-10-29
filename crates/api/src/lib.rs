//! The owner's interface, apart from how it is served: the types it hands out, defined here once (the page's
//! TypeScript types are generated from them by [`typescript`]), the reads that fill them from the database, the bus
//! that carries what changed to the page as it happens, and what the owner asks of the running engine. For the flow
//! page it also explains a compiled flow ([`view`]), tells two apart ([`diff`]), and compiles the flow as it is at a
//! git ref of a project's checkout ([`Sources`]).
//!
//! The interface reads the database through its own read-only connections, beside the engine's writes. The engine's
//! connection tells [`events`] the rows each committed write touched; they become the events the bus hands to every
//! open stream, and an event only names what to read again.

mod bus;
mod control;
mod diff;
mod events;
mod marks;
mod owner;
mod read;
mod settings;
mod source;
mod system;
mod types;
mod typescript;
mod view;

pub use bus::{Bus, Envelope};
pub use control::{Asked, Control, Fresh, Next};
pub use diff::diff;
pub use events::events;
pub use marks::mark_read;
pub use owner::{
    answer, can_start, close_observation, interject, keep_warm_paused, pause, pause_wake, project_named, resume, rule,
    say_to_work, set_keep_warm_paused,
};
pub use read::{
    Error, Flows, architect, compiled_flow, content, conversation, conversations, file, flow_view, flows, inputs_now,
    item, items, round, version_of, work, works,
};
pub use settings::{save_settings, set_level, settings};
pub use source::{Sources, refs};
pub use system::{SystemAsk, system};
pub use types::*;
pub use typescript::typescript;
pub use view::view;
