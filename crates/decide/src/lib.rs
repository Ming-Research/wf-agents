//! What a work does next.
//!
//! This crate is the deciding code of the engine (principle `decide-purely`). A work's state is its blocks; a cell
//! reads some and writes some, and runs again only when a block it reads has changed. Given the flow, a work's records
//! and the named tests of those records, the crate says which cell runs and because of what, which run goes on or is
//! abandoned, or what the work waits for and who can give it. It reads no database, clock or network and changes
//! nothing, and it depends on no other part of the engine (principle `dependencies-one-way`).
//!
//! - [`flow`]: the cells in their order, and the blocks each reads and writes.
//! - [`records`]: what ran and what was written, each list only added to.
//! - [`walk`]: the decision.

pub mod flow;
pub mod records;
pub mod walk;

pub use flow::{BlockId, Cell, CellId, Condition, Flow, FlowFault, TestName};
pub use records::{
    Abandoned, CallId, Called, Cause, Done, Finished, LetGo, Outcome, Outsider, Pause, RecordFault, Records, Returned,
    Round, RunId, Seq, Started, Tests, Who, Write, Written,
};
pub use walk::{Decision, End, Stale, Waited, decide};
