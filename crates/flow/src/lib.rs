//! A flow's definition, read, compiled and compared.
//!
//! A flow is written as a directory of TOML files: `flow.toml` for what belongs to the flow as a whole, among it every
//! block of a work's state and the lines that cells run in, and one file for each cell, saying which blocks run it,
//! which it is given, which it may write and which of those every hand-in of it includes; beside them, in `rules/`, the
//! sets of rules for the agents that a line's system prompt names. This crate checks that definition and compiles it
//! into the flow the deciding code walks, with what the rest of the engine needs to know of each cell and each block,
//! and says what a change from one compiled definition to another changes. Compiling and comparing are functions of
//! what they are given and touch no disk (principle `decide-purely`); reading a directory into the text of its files is
//! the one thing here that does. It depends on the deciding code and on no other part of the engine (principle
//! `dependencies-one-way`).
//!
//! - [`source`]: the files as they are written, and the reading of a directory.
//! - [`compile`](mod@compile): the checks, their faults, and what a definition compiles to.
//! - [`compare`](mod@compare): what changed from one compiled definition to another.

pub mod compare;
pub mod compile;
pub mod source;

pub use compare::{BlockChange, CellChange, Change, Limit, compare};
pub use compile::{
    Block, CellSpec, Compiled, Fault, FaultKind, Given, Has, Limits, Readers, RunsOn, TURNING, Writers, compile,
};
pub use decide::{Test, TestKind};
pub use source::{Effort, FLOW_FILE, GiveFile, InputKind, Kind, RULES_DIR, RunFile, Source, read};
