//! What is kept of a work: its records, and the content the records name. Nothing here is changed once it is
//! written, and nothing is kept that the records already say.
//!
//! The engine's loop adds to a [`Work`] and storage keeps it; both depend on this crate, which depends on the deciding
//! code alone (principle `dependencies-one-way`), so that storage does not depend on the loop.

use std::collections::BTreeMap;

use decide::run::{Failure, Limit, Unmet};
use decide::{BlockId, CallId, Records, RunId, Seq, Who};
use serde::{Deserialize, Serialize};

pub mod sessions;

pub use sessions::{Ran, RanOn, Session, Why};

/// Everything kept of one work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// What the deciding code reads.
    pub records: Records,
    /// The content of each version of each block; the version is named by the place of the entry that wrote it.
    pub contents: BTreeMap<(BlockId, Seq), String>,
    /// What each round that began was given, by its run and its number in the run, the first being one.
    pub given: BTreeMap<(RunId, usize), Given>,
    /// Each time something was filled into the session of a try of a round, in order: what a session holds is read
    /// from here, and from what the rounds that ran in it handed in, never worked out again (design document
    /// `data-and-context`, 13). A try may be filled more than once, as when its session is begun anew under it.
    pub filled: Vec<Filled>,
    /// What came back at the end of each round that ended while its run was open, by its run and its number in the
    /// run, the first being one. A round that ended after its run was abandoned has none: what came back was dropped.
    pub back: BTreeMap<(RunId, usize), Back>,
    /// What each call is.
    pub calls: BTreeMap<CallId, Call>,
    /// The result of each call that has one.
    pub results: BTreeMap<CallId, String>,
    /// What each round that ended took, by its run and its number in the run, the first being one.
    pub spent: BTreeMap<(RunId, usize), Spent>,
    /// When each thing that happened was taken, in order: the first place given to its entries, and the time.
    pub moments: Vec<Moment>,
    /// Each new session of a line that began, in order.
    pub sessions: Vec<Session>,
    /// Each try of an agent's round that began, with what it ran on, in order.
    pub ran: Vec<Ran>,
    /// Why the work's item was cancelled, as whoever cancelled it said, once the records say it was; none where nothing
    /// was said, as by a cancel kept before a cancel said why.
    pub why_cancelled: Option<String>,
}

impl Work {
    /// The next place in the order of what happened to the work: one after the latest entry of its records. Whoever
    /// adds an entry to the records asks for it, and gives the entry this place.
    pub fn tick(&self) -> Seq {
        let r = &self.records;
        let places = (r.started.iter().map(|e| e.at))
            .chain(r.rounds.iter().map(|e| e.at))
            .chain(r.returned.iter().map(|e| e.at))
            .chain(r.called.iter().map(|e| e.at))
            .chain(r.done.iter().map(|e| e.at))
            .chain(r.finished.iter().map(|e| e.at))
            .chain(r.abandoned.iter().map(|e| e.at))
            .chain(r.put_in.iter().map(|e| e.at))
            .chain(r.written.iter().map(|e| e.at))
            .chain(r.let_go.iter().map(|e| e.at))
            .chain(r.pauses.iter().map(|e| e.at))
            .chain(r.cancelled)
            .chain(r.released)
            .chain(self.sessions.iter().map(|s| s.at))
            .chain(self.filled.iter().map(|f| f.at))
            .chain(self.ran.iter().map(|r| r.at));
        Seq(places.map(|at| at.0).max().unwrap_or(0) + 1)
    }

    /// When the entry at this place was added: the time of the thing that happened it belongs to. A place after the
    /// last entry gives the time of the last thing that happened, and a place before the first gives none.
    pub fn time_of(&self, place: Seq) -> Option<Time> {
        self.moments.iter().rev().find(|m| m.from <= place).map(|m| m.at)
    }

    /// A name no run and no call of the work has yet: one after the latest. Whoever starts a run or makes a call asks
    /// for it, and names the run or the call so.
    pub fn name(&self) -> u64 {
        let r = &self.records;
        let names = (r.started.iter().map(|s| s.run.0)).chain(r.called.iter().map(|c| c.call.0));
        names.max().unwrap_or(0) + 1
    }
}

/// A time, as whatever drives the engine reads its clock: milliseconds since the start of 1970. The engine reads no
/// clock itself (principle `outside-behind-interfaces`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Time(pub u64);

/// A thing that happened to the work was taken at this time, and its entries begin at this place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moment {
    pub from: Seq,
    pub at: Time,
}

/// What a round took, as whatever ran it reports (principle `measure-speed-and-cost`). An act of the engine's own
/// takes none of these: its round is reported with all of them at nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spent {
    /// Its cost, in millionths of a dollar.
    pub usd_micros: u64,
    /// The tokens sent that were not read from the cache.
    pub input_tokens: u64,
    /// The tokens sent that were read from the cache.
    pub cached_tokens: u64,
    /// The tokens that came back.
    pub output_tokens: u64,
    /// The context the round ended with, in tokens: what its last call to the model was sent, read from the cache or
    /// not. None where it is not known, and for an act of the engine's own.
    #[serde(default)]
    pub context_tokens: Option<u64>,
    /// How often the agents' program compacted the session during the round.
    #[serde(default)]
    pub compactions: u32,
}

/// What a round is given.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Given {
    /// The first round: the cell's task. Why the cell runs and the version of each block it starts from are the
    /// run's `Started` entry of the records, read there by whoever needs them.
    Task,
    /// The results of the calls the round before asked for.
    Results(Vec<CallId>),
    /// Why what the round before handed in was sent back.
    SentBack(Vec<Unmet>),
}

/// What was filled into the session of a try of a round, at a place of its own in the order of what happened to the
/// work, so that what the session held at any place can be read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filled {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub run: RunId,
    /// The round's number in its run, the first being one.
    pub round: usize,
    /// The try's number in its round, the first being one, as in [`Ran`].
    pub attempt: usize,
    /// What was filled, in the order it was filled; none where the session lacked nothing.
    pub fills: Vec<Fill>,
}

/// One thing filled into a session, because the round's cell is given it and the session lacked it (design document
/// `data-and-context`, 14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fill {
    /// Which work, and which item it is done for, the session is for, by their numbers.
    For { work: u64, item: u64 },
    /// The cell's task, as the flow the round ran on has it; `digest` is the SHA-256 of its text, in hexadecimal, by
    /// which a task the flow changed since is told apart.
    Task { digest: String },
    /// A version of a block, whole: the version is named by the place of the entry that wrote it.
    Whole { block: BlockId, version: Seq },
    /// A version of a block, as what changed in it since an earlier version, which the session held.
    Change { block: BlockId, version: Seq, since: Seq },
    /// The results of these calls: those the cell's last round asked for (design document `data-and-context`, 28).
    Results(Vec<CallId>),
}

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Back {
    /// Calls were asked for. The calls made for them are in the records: those of the run made after this round
    /// ended and before its next began.
    Asked(Vec<Ask>),
    /// These blocks were handed in.
    HandedIn(Vec<Handed>),
    /// Nothing came back: the round failed, for this reason, as whatever carried it out says, with the kind of cause
    /// it told apart. The run's next round is given what this one was given, as the kind's handling lets it (crate
    /// `decide`, module `run`). Records kept before kinds were told apart hold none, and read as a failure that
    /// paused the work, as each did.
    Failed {
        why: String,
        #[serde(default)]
        failure: Failure,
        /// When the account's usage limit resets, where the failure is that limit and its report says when; none
        /// otherwise.
        #[serde(default)]
        resets: Option<Time>,
    },
}

/// A block as a run hands it in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handed {
    pub block: BlockId,
    pub content: String,
    /// Whether the change matters to the cells that read the block.
    pub matters: bool,
    /// The block's value, where it is a choice.
    pub value: Option<String>,
}

/// A call of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Call {
    /// What the run's agent asked for.
    Asked(Ask),
    /// The run reached a limit: the owner lets it go on or cancels the work.
    AtLimit(Limit),
    /// The engine waits until this time before the run's last round, which failed in passing, the `failed`th time in
    /// a row, is tried again. Its result comes from the clock, once the time has come.
    Wait { until: Time, failed: u32 },
}

/// What an agent asks for at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ask {
    /// A tool the engine runs, with what it is run on.
    Tool { name: String, input: String },
    /// A question for a person.
    Person { who: Who, question: String },
}
