//! The cell cases of a flow (design document `testing`, 10 to 17), as they are kept beside the flows: in
//! `cases/<flow>/` of the directory that holds the flows, `common.toml` holding the made items and what the cases share,
//! and a file for each agent's cell holding its cases.
//!
//! A case is the data its cell is given, in the order they were written, with who wrote each: a round of a cell that
//! handed them in, an act of the owner's, or main moving. Then the situation its cell is entered in, which the history
//! leads to: whether the cell ran before, and what of what it watches is new to it. Then the rules its calls get their
//! results by, and what the cell owes. The program that runs a case reaches the situation through the engine itself,
//! the history's rounds handed in as the case says, and runs the cell's line's reading round and the cell for real; this
//! crate holds what decides, and acts on nothing: reading the files, checking them against the compiled flow, and the
//! history as steps (principle `dependencies-one-way`).
//!
//! - [`written`]: the files as written.
//! - [`read`](mod@read): reading a flow's cases directory.
//! - [`checked`]: each case checked against the compiled flow, its names resolved, its history as steps.

pub mod checked;
pub mod read;
pub mod written;

pub use checked::{Case, Cases, Fault, Hand, Of, Round, Step, check};
pub use read::{COMMON, Read, read};
