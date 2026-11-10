//! A work's records, as the deciding code is given them: what ran, what was written, who refused what.
//!
//! They are only added to (principle state-from-records). Nothing here says where the work stands: that is walked.

use std::collections::BTreeMap;

use crate::flow::{CellId, OutputName, ProductId, TestName};

/// A place in the order in which things were recorded: the larger came later. The deciding code is given no clock.
pub type Seq = u64;

/// The number of a run within its work.
pub type RunId = u64;

/// Everything recorded of one work, with the two switches that hold it from outside.
#[derive(Clone, Debug, Default)]
pub struct Records {
    /// The runs, in the order they started.
    pub runs: Vec<Run>,
    /// Every write of every product, in the order written.
    pub versions: Vec<Version>,
    /// Every refusal, in the order made.
    pub refusals: Vec<Refusal>,
    /// Each time the owner let a cell go on after it reached its limit.
    pub let_go: Vec<LetGo>,
    /// The item was cancelled.
    pub cancelled: bool,
    /// The work is paused.
    pub paused: bool,
}

/// One execution of a cell.
#[derive(Clone, Debug)]
pub struct Run {
    pub id: RunId,
    pub cell: CellId,
    pub started: Seq,
    pub state: RunState,
    /// The version of each thing the run was built on.
    pub basis: BTreeMap<ProductId, Built>,
    /// The outputs a condition may read, once the run has finished.
    pub answers: BTreeMap<OutputName, String>,
}

/// How a run stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunState {
    /// An agent's round or a call that needs no person is under way.
    Running,
    /// The run waits at a call for a person; `answered` once that person has given what it waits for.
    Waiting { call: String, who: Who, answered: bool },
    /// The run handed in a return that passed everything required of it.
    Finished { at: Seq },
    /// The run was abandoned while it waited for a person; it counts for nothing.
    Abandoned { at: Seq },
}

/// A version of a product a run was built on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Built {
    pub content: u32,
    pub meaning: u32,
}

/// One write of a product.
#[derive(Clone, Debug)]
pub struct Version {
    pub product: ProductId,
    /// Changes at every write.
    pub content: u32,
    /// Changes only when the write matters to the cells after it.
    pub meaning: u32,
    pub by: Writer,
    pub at: Seq,
    /// The product no longer exists, as when the session that held it was lost.
    pub gone: bool,
}

/// Who wrote a version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Writer {
    Run(RunId),
    Person,
    Outside,
}

/// A "no" to one version of a product.
#[derive(Clone, Debug)]
pub struct Refusal {
    pub by: Refuser,
    pub product: ProductId,
    /// The content version refused: the refusal stands while it is the current one.
    pub content: u32,
    pub why: String,
    pub at: Seq,
}

/// Who refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refuser {
    Run(RunId),
    Person(Who),
}

/// A person who can give what a work waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Who {
    Owner,
    Architect,
}

/// The owner letting a cell go on after it reached its limit: its runs before this are no longer counted.
#[derive(Clone, Debug)]
pub struct LetGo {
    pub cell: CellId,
    pub at: Seq,
}

impl Records {
    /// The product as it stands: its latest version.
    pub fn current(&self, product: &ProductId) -> Option<&Version> {
        self.versions.iter().rev().find(|v| &v.product == product)
    }

    /// The run of this number.
    pub fn run(&self, id: RunId) -> Option<&Run> {
        self.runs.iter().find(|r| r.id == id)
    }

    /// A cell's record: its latest finished run.
    pub fn record(&self, cell: &CellId) -> Option<&Run> {
        self.runs
            .iter()
            .rev()
            .find(|r| &r.cell == cell && matches!(r.state, RunState::Finished { .. }))
    }

    /// The cell's run that waits at a call for a person, if it has one.
    pub fn waiting(&self, cell: &CellId) -> Option<&Run> {
        self.runs
            .iter()
            .rev()
            .find(|r| &r.cell == cell && matches!(r.state, RunState::Waiting { .. }))
    }

    /// Whether a run of this cell wrote a version of the product.
    pub fn wrote(&self, cell: &CellId, product: &ProductId) -> bool {
        self.versions.iter().any(|v| {
            &v.product == product && matches!(&v.by, Writer::Run(id) if self.run(*id).is_some_and(|r| &r.cell == cell))
        })
    }
}

/// What a named test of the records comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Holds,
    /// It does not hold: what is missing, and who can give it.
    Waits {
        what: String,
        who: Who,
    },
}

/// The named tests a flow's conditions and preconditions use. Each is a function of the records alone.
pub trait Tests {
    /// What the test of this name comes to on these records.
    fn test(&self, name: &TestName, records: &Records) -> Outcome;
}
