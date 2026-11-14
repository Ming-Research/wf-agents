//! What is recorded of a work, as the deciding code reads it.
//!
//! Every list here is only added to: an entry, once written, is never changed or taken out. Each entry carries its
//! place in the order of what happened to the work, and no two entries of a work share a place. A run's state is not
//! stored: whether it has a round under way, is at rest, finished or was abandoned is worked out from the entries.

use std::collections::BTreeMap;

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
    /// Each version of a block put into a round while it was under way, which the round was told of beside those it
    /// began with. Records kept before a version reached a round so have none, and read so.
    #[serde(default)]
    pub put_in: Vec<PutIn>,
    /// Every write of a block by a person or from outside the work.
    pub written: Vec<Written>,
    /// Each version of a question put to the owner: asked, asked anew or withdrawn under its name (design document
    /// `data-and-context`, 32, 33). Records kept before questions were data of their own have none, and read so.
    #[serde(default)]
    pub questions: Vec<Questioned>,
    /// Each answer the owner gave to a version of a question (34). Records kept before answers were data of their own
    /// have none, and read so.
    #[serde(default)]
    pub answers: Vec<Answered>,
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
}

impl Records {
    /// Whether the entry at this place wrote a version of the block: a person's or the outside's write of it, or a run
    /// that finished there writing it.
    pub fn wrote(&self, block: &BlockId, at: Seq) -> bool {
        (self.written.iter()).any(|w| &w.block == block && w.at == at)
            || (self.finished.iter()).any(|f| f.at == at && f.writes.iter().any(|w| &w.block == block))
    }

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
            put_in: kept(&self.put_in, |e| e.at, place),
            written: kept(&self.written, |e| e.at, place),
            questions: kept(&self.questions, |e| e.at, place),
            answers: kept(&self.answers, |e| e.at, place),
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
    /// The version the run started from, of each block that runs its cell or that it may write; `None` where there was
    /// none. Of a block that runs the cell, it is the version the round was told of as it began.
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

/// A call of a run: one its return asked for, made as the return was taken and before the run finished, whose result
/// the cell is given when it is entered again; or one the engine makes for a run at rest, which goes on once it has the
/// result, as a wait before a round is tried again or the owner's word at a limit. Runs kept before a return that asks
/// ended its run made the calls they asked for between their rounds, and rested at them.
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
    /// The blocks its return says it has dealt with, by name, in the order it named them, each once: each is a block of
    /// the run's `basis` or one [put in](PutIn) to it. Of each, what the run consumes is the latest version it was told
    /// of: the one it began from, or one put in to it later (design document `data-and-context`, 9). A return that
    /// dealt with nothing names none. None where the return did not say, as no return of the earlier form does and no
    /// record kept before returns said it holds: such a run dealt with every block of its `basis`. That is read from
    /// the run's own record, never from what the flow says runs its cell now, so that a run reads the same after the
    /// flow changes.
    #[serde(default)]
    pub dealt_with: Option<Vec<BlockId>>,
}

/// A block a run wrote as it finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Write {
    pub block: BlockId,
    /// Whether the run hands the version in as changing nothing in meaning, compared with the version the run started
    /// from. Where it does, the version keeps that version's meaning. Not read for a block with a value: a choice
    /// changes its meaning when its value does.
    pub meaning_unchanged: bool,
    /// The block's value, where a condition reads it: a choice such as yes or no.
    pub value: Option<String>,
}

/// A run was abandoned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abandoned {
    pub run: RunId,
    pub at: Seq,
}

/// A version of a block was put into the round of the run that was under way, after the round began, so that the
/// round was told of it: what the owner says reaches a round so (design document `data-and-context`, 41). The version
/// was written before it was put in, and the round was under way then. A version the round was never told of stays
/// new to its cell, however the round ends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PutIn {
    pub run: RunId,
    pub block: BlockId,
    /// The version put into the round, named by the place of the entry that wrote it.
    pub version: Seq,
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

/// A version of a question put to the owner, named by the place of this entry: what the walk reads of it. What it asks
/// and its options are kept with what came back from the round that brought it. A question has a name no other
/// question of the work has, and belongs to the cell of its latest version: one cell's alone where an agent asks it,
/// the cell of the round at whose end the engine found what it asks where the engine does. A version that withdraws it
/// leaves it standing no more.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Questioned {
    pub name: String,
    /// The cell it belongs to: the cell of the run whose return asked it, or whose round's end the engine asked it at.
    pub cell: CellId,
    /// The run whose return brought it.
    pub run: RunId,
    /// The engine itself asked it, from what it found of the work as the run's round ended, and not the run's agent.
    pub engine: bool,
    /// The cell waits for the answer: the work goes past the cell to no later one until this version is answered.
    pub waits: bool,
    /// This version withdraws the question.
    pub withdrawn: bool,
    pub at: Seq,
}

/// The owner's answer to a version of a question: the option taken, counted from one, and why, where the owner said.
/// The last answer to a question's latest version is its answer; one to an earlier version answers it no more.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answered {
    pub question: String,
    /// The version answered, named by the place of the entry that wrote it.
    pub version: Seq,
    pub option: u64,
    pub why: Option<String>,
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
    /// It had no record: a cell ran once before anything else ran it, as runs started before the walk chose by what is
    /// new say. The walk no longer gives it: a cell runs only for what is new to it.
    NoRecord,
    /// A block that runs the cell is new to it: it has a version, changing its meaning, later than the one the cell
    /// consumed, that the cell did not write itself (design document `data-and-context`, 6). `was` is the version the
    /// cell consumed, `None` where it consumed none; the run starts from the latest, as its `basis` says. `behind` says
    /// every such new version was written by a cell after this one: a judging cell's opinion on what it wrote, for one.
    Changed {
        block: BlockId,
        was: Option<Seq>,
        behind: bool,
    },
    /// The calls the cell's earlier runs asked for, as their returns were taken, all have their results, which no
    /// run of the cell was given yet (design document `data-and-context`, 28). The run is given them.
    Results { calls: Vec<CallId> },
}

impl Cause {
    /// Whether the cause is no way round: a first run, or a version that a person, the outside or a cell before this
    /// one wrote.
    pub fn forward(&self) -> bool {
        match self {
            Cause::NoRecord => true,
            Cause::Changed { behind, .. } => !behind,
            Cause::Results { .. } => false,
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
    /// What was put in at this place is no version of its block.
    PutInNoVersion(Seq),
    /// What was put in at this place is a version written at that place or after it.
    PutInBeforeWritten(Seq),
    /// What was put in at this place was put into a run that had no round under way then.
    PutInNoRound(Seq),
    /// The run's return names, as dealt with, a block that is neither in its `basis` nor was put in to it.
    DealtWithUnknown(RunId),
    /// The run's return names one block twice as dealt with.
    DealtWithTwice(RunId),
    /// The call was made after its run had finished or was abandoned: a run makes its calls before it ends.
    CalledAfterEnd(CallId),
    /// The answer at this place names no version of its question, asking it, written before it.
    AnswerNoVersion(Seq),
}
