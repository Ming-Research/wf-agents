//! A work's records, as the deciding code is given them: what ran, what was written, who refused what.
//!
//! Every list is only added to, and each entry carries its place in the order of what happened (principle
//! state-from-records). Nothing here says where the work stands or how a run stands: both are worked out.

use std::collections::BTreeMap;

use crate::flow::{CellId, OutputName, ProductId, TestName};

/// A place in the order in which things were recorded: the larger came later. The deciding code is given no clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(pub u64);

/// The number of a run within its work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(pub u64);

/// The number of a refusal within its work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefusalId(pub u64);

/// The number of a call within its work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallId(pub u64);

/// Everything recorded of one work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records {
    /// Each run, as it started.
    pub started: Vec<Started>,
    /// Each run that handed in a return which passed everything required of it.
    pub finished: Vec<Finished>,
    /// Each run that was abandoned while it waited for a person; it counts for nothing.
    pub abandoned: Vec<Abandoned>,
    /// Each time a run came to wait at a call for a person.
    pub asked: Vec<Asked>,
    /// Each such call the person answered.
    pub answered: Vec<Answered>,
    /// Every write of every product.
    pub versions: Vec<Version>,
    /// Every refusal.
    pub refusals: Vec<Refusal>,
    /// Each time the owner let a cell go on after it reached its limit.
    pub let_go: Vec<LetGo>,
    /// Each time the work was paused or let on again.
    pub pauses: Vec<Pause>,
    /// When the item was cancelled, if it was.
    pub cancelled: Option<Seq>,
}

/// A run as it started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub run: RunId,
    pub cell: CellId,
    pub at: Seq,
    /// The version of each thing the run was built on.
    pub basis: BTreeMap<ProductId, Built>,
    /// The refusals the run was given: those that were among its reasons to run.
    pub given: Vec<RefusalId>,
}

/// A run's end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    pub run: RunId,
    pub at: Seq,
    /// The outputs a condition may read.
    pub answers: BTreeMap<OutputName, String>,
}

/// A run abandoned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Abandoned {
    pub run: RunId,
    pub at: Seq,
}

/// A run coming to wait at a call for a person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    pub call: CallId,
    pub run: RunId,
    pub what: String,
    pub who: Who,
    pub at: Seq,
}

/// The person's answer to a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answered {
    pub call: CallId,
    pub at: Seq,
}

/// A version of a product a run was built on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Built {
    pub content: u32,
    pub meaning: u32,
}

/// One write of a product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub product: ProductId,
    /// Changes at every write.
    pub content: u32,
    /// Changes only when the write matters to the cells after it.
    pub meaning: u32,
    pub by: Writer,
    pub at: Seq,
    /// The product no longer exists, as when the session that held it was lost. Such an entry is no write.
    pub gone: bool,
}

/// Who wrote a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writer {
    Run(RunId),
    Person,
    Outside,
}

/// A "no" to one version of a product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub id: RefusalId,
    pub by: Refuser,
    pub product: ProductId,
    /// The content version refused.
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LetGo {
    pub cell: CellId,
    pub at: Seq,
}

/// The work paused, or let on again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pause {
    pub paused: bool,
    pub at: Seq,
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
