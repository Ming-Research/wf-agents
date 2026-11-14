//! A flow's definition, read and compiled.
//!
//! A flow is written as a directory of TOML files, laid out one thing to a file: `flow.toml` for what belongs to the
//! flow as a whole, and a file for each line, each block of a work's state, each named test and each cell, the cell's
//! saying what runs it, what it is given, which blocks it may write and which of those every hand-in of it includes.
//! What every flow shares is beside the flows: the engine's own data, what runs a cell, and what the engine says to
//! every agent. A flow written before that layout, its lines, blocks and tests in `flow.toml` and its sets of rules in
//! `rules/`, is read as well. This crate checks a definition and compiles it into the flow the deciding code walks,
//! with what the rest of the engine needs to know of each cell and each block. Compiling is a function of what it is
//! given and touches no disk (principle `decide-purely`); reading a directory into the text of its files, and reading
//! what the engine says to every agent, are the things here that do. It depends on the deciding code and on no other part of the engine
//! (principle `dependencies-one-way`).
//!
//! - [`source`]: the files as they are written, and the reading of a directory.
//! - [`compile`](mod@compile): the checks, their faults, and what a definition compiles to.
//! - [`agents`]: what the engine says to every agent, whatever the flow, as written beside the flows.

pub mod agents;
pub mod compile;
pub mod source;

pub use compile::{
    Block, CellSpec, Compiled, Fault, FaultKind, Given, Has, Limits, Readers, RunsOn, TURNING, Trigger, Writers,
    compile,
};
pub use decide::{Test, TestKind};
pub use source::{
    BLOCKS_DIR, CELLS_DIR, DATA_DIR, Effort, FLOW_FILE, GiveFile, InputKind, Kind, LINES_DIR, OF_A_CELL, OF_A_DATUM,
    OWNER, RULES_DIR, RunFile, Source, TESTS_DIR, TRIGGERS_DIR, Texts, read, rules_file,
};
