//! What is kept of a work: its records, and the content the records name.

use std::collections::BTreeMap;

use decide::run::{Limit, Unmet};
use decide::{BlockId, CallId, Cause, Records, RunId, Seq, Who};

/// Everything kept of one work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// What the deciding code reads.
    pub records: Records,
    /// The content of each version of each block; the version is named by the place of the entry that wrote it.
    pub contents: BTreeMap<(BlockId, Seq), String>,
    /// What each round of each run was given, in order.
    pub given: BTreeMap<RunId, Vec<Given>>,
    /// What came back at the end of each round of each run, in order.
    pub back: BTreeMap<RunId, Vec<Back>>,
    /// What each call is.
    pub calls: BTreeMap<CallId, Call>,
    /// The result of each call that has one.
    pub results: BTreeMap<CallId, String>,
    /// The last place given to an entry.
    place: u64,
    /// The last name given to a run or a call.
    name: u64,
}

impl Work {
    /// The next place in the order of what happened to the work.
    pub(crate) fn tick(&mut self) -> Seq {
        self.place += 1;
        Seq(self.place)
    }

    /// A name no run and no call of the work has yet.
    pub(crate) fn name(&mut self) -> u64 {
        self.name += 1;
        self.name
    }
}

/// What a round is given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Given {
    /// The first round: the cell's task, with why the cell runs and the version of each block it starts from.
    Task {
        because: Vec<Cause>,
        basis: BTreeMap<BlockId, Option<Seq>>,
    },
    /// The results of the calls the round before asked for.
    Results(Vec<CallId>),
    /// Why what the round before handed in was sent back.
    SentBack(Vec<Unmet>),
}

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Back {
    /// Calls were asked for: what was asked, and the calls made for it, none until the engine makes them.
    Asked { asks: Vec<Ask>, calls: Vec<CallId> },
    /// These blocks were handed in.
    HandedIn(Vec<Handed>),
}

/// A block as a run hands it in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handed {
    pub block: BlockId,
    pub content: String,
    /// Whether the change matters to the cells that read the block.
    pub matters: bool,
    /// The block's value, where it is a choice.
    pub value: Option<String>,
}

/// A call of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// What the run's agent asked for.
    Asked(Ask),
    /// The run reached a limit: the owner lets it go on or cancels the work.
    AtLimit(Limit),
}

/// What an agent asks for at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// A tool the engine runs, with what it is run on.
    Tool { name: String, input: String },
    /// A question for a person.
    Person { who: Who, question: String },
}
