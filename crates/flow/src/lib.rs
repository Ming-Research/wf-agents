//! A flow's definition, read and compiled.
//!
//! A flow is written as a directory of TOML files: `flow.toml` for what belongs to the flow as a whole, among it every
//! block of a work's state, and one file for each cell, saying which blocks it reads and which it may write. This
//! crate checks that definition and compiles it into the flow the deciding code walks, with what the rest of the
//! engine needs to know of each cell and each block. Compiling is a function of the files' text and of the tests the
//! engine has, and touches no disk (principle `decide-purely`); reading a directory into that text is the one thing
//! here that does. It depends on the deciding code and on no other part of the engine (principle
//! `dependencies-one-way`).
//!
//! - [`source`]: the files as they are written, and the reading of a directory.
//! - [`compile`](mod@compile): the checks, their faults, and what a definition compiles to.

pub mod compile;
pub mod source;

pub use compile::{Block, CellSpec, Compiled, Fault, FaultKind, Limits, Readers, Writers, compile};
pub use source::{FLOW_FILE, Kind, Source, read};
