//! What is recorded of a work, as the deciding code reads it.
//!
//! Every list here is only added to: an entry, once written, is never changed or taken out. Each entry carries its
//! place in the order of what happened to the work, and no two entries of a work share a place. A run's state is not
//! stored: whether it has a round under way, is at rest, finished or was abandoned is worked out from the entries.

use std::collections::BTreeMap;

use crate::flow::{CellId, OutputName, ProductId, TestName};

/// The place of an entry in the order of what happened to a work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(pub u64);

/// The name of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(pub u64);

/// The name of a refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefusalId(pub u64);

/// The name of a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallId(pub u64);

/// Everything recorded of one work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records {
    /// Each run that started.
    pub started: Vec<Started>,
    /// Each round of a run that began: the agent was given something, or the engine began an act of its own.
    pub rounds: Vec<Round>,
    /// Each round that ended: the agent returned, or the act was done.
    pub returned: Vec<Returned>,
    /// Each call a run rests at until it has its result.
    pub called: Vec<Called>,
    /// Each call that has its result.
    pub done: Vec<Done>,
    /// Each run that handed in its return and passed everything the flow requires of it, with what it wrote.
    pub finished: Vec<Finished>,
    /// Each run that was abandoned, leaving no record.
    pub abandoned: Vec<Abandoned>,
    /// Every write of a product by a person or from outside the work.
    pub written: Vec<Written>,
    /// Every loss of a product: a session that no longer exists, for one.
    pub lost: Vec<Lost>,
    /// Every refusal.
    pub refusals: Vec<Refusal>,
    /// Each time the owner let a cell go on after it reached its limit.
    pub let_go: Vec<LetGo>,
    /// Each time the work was paused or let on again; the latest says which it is.
    pub pauses: Vec<Pause>,
    /// When the work's item was cancelled, if it was.
    pub cancelled: Option<Seq>,
}

/// A run began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub run: RunId,
    pub cell: CellId,
    pub at: Seq,
    /// Why the run was started, as the decision gave it.
    pub because: Vec<Cause>,
    /// The version the run started from, of each product its cell depends on or may write; `None` where there was
    /// none.
    pub basis: BTreeMap<ProductId, Option<Seq>>,
    /// The refusals the run was given: those that stood, with force, against a product its cell may write.
    pub given: Vec<RefusalId>,
}

/// A round of a run began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    pub run: RunId,
    pub at: Seq,
}

/// The round of the run that was under way ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Returned {
    pub run: RunId,
    pub at: Seq,
}

/// A run rests at a call until the call has its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Called {
    pub call: CallId,
    pub run: RunId,
    /// The person who gives the result; `None` when the engine makes the call itself, as it does a tool.
    pub on: Option<Who>,
    pub at: Seq,
}

/// A call has its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
    pub call: CallId,
    pub at: Seq,
}

/// A run handed in its return, which passed everything the flow requires. A run writes its products here and nowhere
/// else: nothing of a run that has not finished is a version of anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    pub run: RunId,
    pub at: Seq,
    /// The answers a condition may read.
    pub answers: BTreeMap<OutputName, String>,
    /// The products the run wrote. Each is a version of its product, named by this entry's place.
    pub writes: Vec<Write>,
}

/// A product a run wrote as it finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Write {
    pub product: ProductId,
    /// Whether the change matters to the cells that depend on the product, compared with the version the run started
    /// from. Where it does not, the version keeps that version's meaning.
    pub matters: bool,
}

/// A run was abandoned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Abandoned {
    pub run: RunId,
    pub at: Seq,
}

/// A person, or something outside the work, wrote a product. Such a write always changes the product's meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub product: ProductId,
    pub by: Outsider,
    pub at: Seq,
}

/// Who writes a product without running a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outsider {
    Person,
    Outside,
}

/// A product was lost. A loss changes no meaning: the cell that wrote the product builds it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lost {
    pub product: ProductId,
    pub at: Seq,
}

/// A "no" to a version of a product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub id: RefusalId,
    pub by: Refuser,
    pub product: ProductId,
    /// The version refused, named by the place of the entry that wrote it.
    pub version: Seq,
    pub at: Seq,
}

/// Who refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refuser {
    /// A cell, by this run of it.
    Run(RunId),
    Person(Who),
}

/// A person who can give what a work waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Who {
    Owner,
    Architect,
}

/// The owner let the cell go on after it reached its limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LetGo {
    pub cell: CellId,
    pub at: Seq,
}

/// The work was paused, or let on again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pause {
    pub paused: bool,
    pub at: Seq,
}

/// Why a cell runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    /// It has no record.
    NoRecord,
    /// A product its record wrote was lost.
    Gone { product: ProductId },
    /// Something it depends on changed in meaning since its record was built. `before` says whether the version that
    /// now stands came from before the cell: from a person, from outside, from a cell before it, or from the path
    /// having changed.
    Changed { product: ProductId, before: bool },
    /// A refusal its record made no longer stands: what it refused was written anew.
    Answered { refusal: RefusalId, product: ProductId },
    /// A refusal stands, with force, against a product the cell answers for.
    Refused {
        refusal: RefusalId,
        product: ProductId,
        by: Refuser,
    },
}

impl Cause {
    /// Whether the cause came from before the cell or from a person, and not from what cells after it did. A run
    /// none of whose causes did is a way round.
    pub fn forward(&self) -> bool {
        match self {
            Cause::NoRecord => true,
            Cause::Changed { before, .. } => *before,
            Cause::Refused { by, .. } => matches!(by, Refuser::Person(_)),
            Cause::Gone { .. } | Cause::Answered { .. } => false,
        }
    }
}

/// What a named test of the records comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The test holds.
    Holds,
    /// It does not: the person who can make it hold.
    Waits { who: Who },
}

/// The named tests of a work's records: the conditions that are no agent's answer, and the preconditions. Each is
/// worked out from the records as they are, every time it is asked.
pub trait Tests {
    fn test(&self, name: &TestName, records: &Records) -> Outcome;
}

/// What is wrong with records that cannot be those of a work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordFault {
    /// Two runs started with this name.
    RunTwice(RunId),
    /// An entry names a run that never started.
    UnknownRun(RunId),
    /// A result names a call that was never made.
    UnknownCall(CallId),
    /// The run finished twice.
    FinishedTwice(RunId),
    /// The run both finished and was abandoned.
    FinishedAndAbandoned(RunId),
    /// More rounds of the run ended than began.
    ReturnedWithoutRound(RunId),
    /// Two entries share this place.
    PlaceTwice(Seq),
}
