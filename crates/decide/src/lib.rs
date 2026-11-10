//! What a work does next.
//!
//! This crate is the deciding code of the engine (principle `decide-purely`): given the flow, a work's records and the
//! named tests of those records, it says which cell runs and because of what, which run goes on or is abandoned, or
//! what the work waits for and who can give it. It reads no database, clock or network and changes nothing, and it
//! depends on no other part of the engine (principle `dependencies-one-way`).
//!
//! - [`flow`]: the cells in their order and how they are joined.
//! - [`records`]: what ran, what was written and who refused what, each list only added to.
//! - [`walk`]: the decision.

pub mod flow;
pub mod records;
pub mod walk;

pub use flow::{Cell, CellId, Condition, Flow, FlowFault, OutputName, ProductId, TestName};
pub use records::{
    Abandoned, CallId, Called, Cause, Done, Finished, LetGo, Lost, Outcome, Outsider, Pause, RecordFault, Records,
    Refusal, RefusalId, Refuser, Returned, Round, RunId, Seq, Started, Tests, Who, Write, Written,
};
pub use walk::{Decision, End, Stale, Waited, current, decide};
