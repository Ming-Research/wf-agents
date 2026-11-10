//! What a work does next.
//!
//! This crate is the deciding code of the engine (principle `decide-purely`). A work's state is its blocks; a cell
//! reads some and writes some, and runs again only when a block it reads has changed in meaning. Given the flow, with
//! the named tests its cells' conditions and preconditions are, and a work's records, the crate says which cell runs
//! and because of what, which run goes on or is abandoned, or what the work waits for and who can give it. It reads
//! no database, clock or network and changes nothing, and it depends on no other part of the engine (principle
//! `dependencies-one-way`).
//!
//! - [`flow`]: the cells in their order, the blocks each reads and writes, and the named tests.
//! - [`records`]: what ran and what was written, each list only added to.
//! - [`walk`]: the decision: which cell runs, which run goes on.
//! - [`run`]: what a run that goes on does next.
//! - [`view`](mod@view): a work as it is now, for whoever shows it.

pub mod flow;
pub mod records;
pub mod run;
pub mod view;
pub mod walk;

pub use flow::{BlockId, Cell, CellId, Condition, Flow, FlowFault, Test, TestKind, TestName};
pub use records::{
    Abandoned, CallId, Called, Cause, Done, Finished, LetGo, Outsider, Pause, RecordFault, Records, Returned, Round,
    RunId, Seq, Started, Who, Write, Written,
};
pub use view::{CellView, InputView, OpenRun, VersionView, View, Writer, versions, view};
pub use walk::{Abandoning, Decision, End, Waited, decide, paused, stale};
