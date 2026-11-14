//! What a work does next.
//!
//! This crate is the deciding code of the engine (principle `decide-purely`). A work's state is its data, each with
//! versions; a cell names the blocks that run it and those it writes, and runs when one that runs it has a version new
//! to it, which a run of it consumes by saying, as its return is taken, that it dealt with it (design document
//! `data-and-context`). Given the flow, with the named tests its cells' conditions and preconditions are, and a work's
//! records, the crate says which cell runs and because of what, which run goes on or is abandoned, or what the work
//! waits for and who can give it. It reads no database, clock or network and changes nothing, and it depends on no
//! other part of the engine (principle `dependencies-one-way`).
//!
//! - [`flow`]: the cells in their order, the blocks that run each and those it writes, and the named tests.
//! - [`records`]: what ran and what was written, each list only added to.
//! - [`questions`]: the questions put to the owner and the answers, as they stand.
//! - [`walk`]: the decision: which cell runs, which run goes on.
//! - [`run`]: what a run that goes on does next.
//! - [`room`]: what the engine as a whole leaves a work room for: the owner's pause of everything, and the slots.
//! - [`view`](mod@view): a work as it is now, for whoever shows it.

pub mod flow;
pub mod questions;
pub mod records;
pub mod room;
pub mod run;
pub mod view;
pub mod walk;

pub use flow::{BlockId, Cell, CellId, Condition, Flow, FlowFault, Outsiders, Test, TestKind, TestName};
pub use records::{
    Abandoned, Answered, CallId, Called, Cause, Done, Finished, LetGo, Outsider, Pause, Pauser, PutIn, Questioned,
    RecordFault, Records, Returned, Round, RunId, Seq, Started, Who, Write, Written,
};
pub use room::{Holder, Room, begun, taking_slot};
pub use view::{CellView, InputView, OpenRun, VersionView, View, Writer, versions, view, view_in};
pub use walk::{Abandoning, Decision, End, Maker, Waited, decide, decide_in, hold, hold_in, owed, pause, paused, told};
