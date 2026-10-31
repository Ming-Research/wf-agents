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
mod talk;
mod types;
mod typescript;
mod view;

pub use bus::{Bus, Envelope};
pub use control::{Asked, Control, Fresh, Next};
pub use diff::diff;
pub use events::events;
pub use marks::{mark_read, read_to};
pub use owner::{
    answer, can_start, close_observation, fault_forwarding, forward_fault, images, interject, keep_warm_paused, pause,
    pause_architect, project_named, remove_dependency, reply_to_question, resume, rule, say_to_projects, say_to_work,
    set_fault_forwarding, set_keep_warm_paused, start_over,
};
pub use read::{
    Error, Flows, ROUNDS_WINDOW, ROUNDS_WINDOW_MOST, RoundsWindow, architect, architect_rounds, attachment,
    compiled_flow, content, file, flow_view, flows, inputs_now, item, items, observations_by_project, round,
    version_of, work, works,
};
pub use settings::{accounts, save_settings, set_level, settings, switch_account};
pub use source::{Sources, refs};
pub use system::{SystemAsk, system};
pub use talk::{Talks, conversation, conversations};
pub use types::*;
pub use typescript::typescript;
pub use view::{flow_lines, view};
