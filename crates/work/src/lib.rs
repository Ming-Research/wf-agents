//! What is kept of a work: its records, and the content the records name. Nothing here is changed once it is
//! written, and nothing is kept that the records already say.
//!
//! The engine's loop adds to a [`Work`] and storage keeps it; both depend on this crate, which depends on the deciding
//! code alone (entry `engine-in-layers`), so that storage does not depend on the loop.

use std::collections::BTreeMap;

use decide::run::{Limit, Unmet};
use decide::{BlockId, CallId, Records, RunId, Seq, Who};
use serde::{Deserialize, Serialize};

/// Everything kept of one work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// What the deciding code reads.
    pub records: Records,
    /// The content of each version of each block; the version is named by the place of the entry that wrote it.
    pub contents: BTreeMap<(BlockId, Seq), String>,
    /// What each round that began was given, by its run and its number in the run, the first being one.
    pub given: BTreeMap<(RunId, usize), Given>,
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
            .chain(r.written.iter().map(|e| e.at))
            .chain(r.let_go.iter().map(|e| e.at))
            .chain(r.pauses.iter().map(|e| e.at))
            .chain(r.cancelled);
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

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Back {
    /// Calls were asked for. The calls made for them are in the records: those of the run made after this round
    /// ended and before its next began.
    Asked(Vec<Ask>),
    /// These blocks were handed in.
    HandedIn(Vec<Handed>),
    /// Nothing came back: the round failed, for this reason, as whatever carried it out says. The engine paused the
    /// work as it took this, and the run's next round is given what this one was given.
    Failed { why: String },
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
}

/// What an agent asks for at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ask {
    /// A tool the engine runs, with what it is run on.
    Tool { name: String, input: String },
    /// A question for a person.
    Person { who: Who, question: String },
}
