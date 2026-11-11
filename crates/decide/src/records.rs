//! What is recorded of a work, as the deciding code reads it.
//!
//! Every list here is only added to: an entry, once written, is never changed or taken out. Each entry carries its
//! place in the order of what happened to the work, and no two entries of a work share a place. A run's state is not
//! stored: whether it has a round under way, is at rest, finished or was abandoned is worked out from the entries.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::flow::{BlockId, CellId};

/// The place of an entry in the order of what happened to a work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Seq(pub u64);

/// The name of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RunId(pub u64);

/// The name of a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CallId(pub u64);

/// Everything recorded of one work.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Each run that handed in its return and passed everything the flow requires of it, with the blocks it wrote.
    pub finished: Vec<Finished>,
    /// Each run that was abandoned, leaving no record.
    pub abandoned: Vec<Abandoned>,
    /// Every write of a block by a person or from outside the work.
    pub written: Vec<Written>,
    /// Each time the owner let a cell go on after it reached its limit.
    pub let_go: Vec<LetGo>,
    /// Each time the work was paused or let on again; the latest says which it is.
    pub pauses: Vec<Pause>,
    /// When the work's item was cancelled, if it was.
    pub cancelled: Option<Seq>,
    /// When what the work held outside the engine was let go, once it had ended and nothing of it was under way: its
    /// worktree, and its pull request where it did not merge. Records kept before the engine let go of anything have
    /// none, and read so.
    #[serde(default)]
    pub released: Option<Seq>,
    /// The versions written with nothing in them, each by its block and the place of the entry that wrote it: a block
    /// written blank says nothing, as one never written does. It is told by the content each entry names, which the
    /// deciding code is not given; records kept before it was told read as having none.
    #[serde(default)]
    pub blank: BTreeSet<(BlockId, Seq)>,
}

impl Records {
    /// The records as they stood before this place: every entry at an earlier place, and none at it or after. Records
    /// are only added to, so these are what was recorded then, and what was decided then is decided again from them.
    pub fn before(&self, place: Seq) -> Records {
        fn kept<T: Clone>(entries: &[T], at: impl Fn(&T) -> Seq, place: Seq) -> Vec<T> {
            entries.iter().filter(|e| at(e) < place).cloned().collect()
        }
        Records {
            started: kept(&self.started, |e| e.at, place),
            rounds: kept(&self.rounds, |e| e.at, place),
            returned: kept(&self.returned, |e| e.at, place),
            called: kept(&self.called, |e| e.at, place),
            done: kept(&self.done, |e| e.at, place),
            finished: kept(&self.finished, |e| e.at, place),
            abandoned: kept(&self.abandoned, |e| e.at, place),
            written: kept(&self.written, |e| e.at, place),
            let_go: kept(&self.let_go, |e| e.at, place),
            pauses: kept(&self.pauses, |e| e.at, place),
            cancelled: self.cancelled.filter(|at| *at < place),
            released: self.released.filter(|at| *at < place),
        }
    }
}

/// A run began.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Started {
    pub run: RunId,
    pub cell: CellId,
    pub at: Seq,
    /// Why the run was started, as the decision gave it.
    pub because: Vec<Cause>,
    /// The version the run started from, of each block its cell reads or may write; `None` where there was none.
    pub basis: BTreeMap<BlockId, Option<Seq>>,
}

/// A round of a run began.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    pub run: RunId,
    pub at: Seq,
}

/// The round of the run that was under way ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Returned {
    pub run: RunId,
    pub at: Seq,
}

/// A run rests at a call until the call has its result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Called {
    pub call: CallId,
    pub run: RunId,
    /// The person who gives the result; `None` when the engine makes the call itself, as it does a tool.
    pub on: Option<Who>,
    pub at: Seq,
}

/// A call has its result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    pub call: CallId,
    pub at: Seq,
}

/// A run handed in its return, which passed everything the flow requires. A run writes its blocks here and nowhere
/// else: nothing of a run that has not finished is a version of anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finished {
    pub run: RunId,
    pub at: Seq,
    /// The blocks the run wrote. Each is a version of its block, named by this entry's place.
    pub writes: Vec<Write>,
}

/// A block a run wrote as it finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Write {
    pub block: BlockId,
    /// Whether the change matters to the cells that read the block, compared with the version the run started from.
    /// Where it does not, the version keeps that version's meaning. Not read for a block with a value: a choice
    /// changes its meaning when its value does.
    pub matters: bool,
    /// The block's value, where a condition reads it: a choice such as yes or no.
    pub value: Option<String>,
}

/// A run was abandoned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abandoned {
    pub run: RunId,
    pub at: Seq,
}

/// A person, or something outside the work, wrote a block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Written {
    pub block: BlockId,
    pub by: Outsider,
    /// Whether what the outside wrote changes the block's meaning: a changed specification of a cell does when the
    /// owner says so. Not read for a person's prose, which always changes the meaning, nor for a block with a value:
    /// a choice changes its meaning when its value does, so a person confirming what was recommended changes none.
    pub matters: bool,
    /// The block's value, where a condition reads it.
    pub value: Option<String>,
    pub at: Seq,
}

/// Who writes a block without running a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outsider {
    Person,
    Outside,
}

/// A person who can give what a work waits for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Who {
    Owner,
    Architect,
}

/// The owner let the cell go on after it reached its limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LetGo {
    pub cell: CellId,
    pub at: Seq,
}

/// The work was paused, or let on again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pause {
    pub paused: bool,
    pub at: Seq,
    /// Who paused the work, or let it on. Records kept before the engine paused works itself hold only the owner's
    /// pauses, and read so.
    #[serde(default)]
    pub by: Pauser,
    /// Why the work was paused, where whoever paused it said: the engine always says, the owner does not. None for a
    /// letting on.
    #[serde(default)]
    pub why: Option<String>,
}

/// Who pauses a work: the owner, or the engine itself, as when a round failed or the work spent more than it may.
/// Only the owner lets a work on again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pauser {
    #[default]
    Owner,
    Engine,
}

/// Why a cell runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cause {
    /// It has no record.
    NoRecord,
    /// A block it reads has another meaning than the one its record knows. `was` is the version the record knows:
    /// the one its run started from, or the one it wrote itself; the run now starts from the current one. `behind`
    /// says every change of the block's meaning since that version was written by a cell after this one: a judging
    /// cell's opinion on what it wrote, for one.
    Changed {
        block: BlockId,
        was: Option<Seq>,
        behind: bool,
    },
}

impl Cause {
    /// Whether the cause is no way round: a first run, or a change of meaning that a person, the outside or a cell
    /// before this one made.
    pub fn forward(&self) -> bool {
        match self {
            Cause::NoRecord => true,
            Cause::Changed { behind, .. } => !behind,
        }
    }
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
    /// The run wrote one block twice as it finished.
    WroteTwice(RunId),
    /// The call has two results.
    DoneTwice(CallId),
    /// The run both finished and was abandoned.
    FinishedAndAbandoned(RunId),
    /// More rounds of the run ended than began.
    ReturnedWithoutRound(RunId),
    /// Two entries share this place.
    PlaceTwice(Seq),
}
