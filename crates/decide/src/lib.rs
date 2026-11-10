//! What a work does next.
//!
//! This crate is the deciding code of the engine (principle `decide-purely`): given the flow, the configuration a work
//! runs on and the work's records, it says which cell runs and because of what, or what the work waits for and who can
//! give it. It reads no database, clock or network and changes nothing, and it depends on no other part of the engine
//! (principle `dependencies-one-way`).
//!
//! - [`flow`]: the cells in their order and how they are joined.
//! - [`records`]: what ran, what was written and who refused what.
//! - [`walk`]: the decision.

pub mod flow;
pub mod records;
pub mod walk;

pub use flow::{Cell, CellId, Condition, Flow, OutputName, ProductId, TestName};
pub use records::{
    Built, LetGo, Outcome, Records, Refusal, Refuser, Run, RunId, RunState, Seq, Tests, Version, Who, Writer,
};
pub use walk::{Cause, Decision, End, Stale, Waited, decide};
