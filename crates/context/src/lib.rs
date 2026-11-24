//! The context manager, and the message a cell's agent is sent as the engine enters the cell (design document
//! `data-and-context`, 12 to 20). Both are functions of the flow, the files beside it and the work's records, with no
//! I/O: what a round is sent is worked out from what its session holds, which the records say, and the same round
//! given again is sent the same bytes. Whatever carries a round out is given the message and what it fills, and
//! records what it filled; the agent is given no way to the engine's records (principle
//! `agents-run-apart-from-the-engine`). The manager depends on the deciding code, the flow and what is kept of a work
//! alone; only [`sim`] and [`witness`], which run works through the engine's own loop to find the situations a round
//! can stand in, depend on the engine as well, and the engine does not depend on this crate (principle
//! `dependencies-one-way`).
//!
//! - [`manager`]: the session a round runs in, what it holds, and what is filled into it.
//! - [`message`]: the message, built from the flow's files for the situation an entry is in: by the engine with every
//!   place filled, and for the page with the places of a run open.
//! - [`situations`]: the situations an entry of a cell was found in, for whoever reads what it is sent in each.
//! - [`sim`]: a work taken one thing at a time through the engine's loop, with its rounds entered as a carrier enters
//!   them.
//! - [`witness`]: the search for histories of works that reach each situation, with the script of each.
//! - [`diff`]: what changed between two versions, as a datum given `last_and_latest` is sent it.
//! - [`text`]: a text of the files, its places apart from what is sent as written.

pub mod diff;
pub mod manager;
pub mod message;
pub mod sim;
pub mod situations;
pub mod text;
pub mod witness;

pub use manager::{Author, Carried, Changes, Entered, For, author, author_from, enter, situation_of};
pub use message::{Defined, Form, Main, Sent, Situation, Stands, Values, defined, message};
pub use text::Piece;
