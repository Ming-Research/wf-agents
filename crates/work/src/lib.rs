//! What is kept of a work: its records, and the content the records name. Nothing here is changed once it is
//! written, and nothing is kept that the records already say.
//!
//! The engine's loop adds to a [`Work`] and storage keeps it; both depend on this crate, which depends on the deciding
//! code alone (principle `dependencies-one-way`), so that storage does not depend on the loop.

use std::collections::BTreeMap;

use decide::run::{Failure, Limit, Unmet};
use decide::{BlockId, CallId, Records, RunId, Seq, Who};
use serde::{Deserialize, Serialize};

pub mod filled;
pub mod flows;
pub mod measured;
pub mod observations;
pub mod sessions;
pub mod time;

pub use filled::{Fill, FillFault, Filled};
pub use flows::{OnFlow, Read, Version, version_of};
pub use measured::{BesideKind, Forked, Measured, Measures, Pinged, SpentBeside};
pub use observations::Observed;
pub use sessions::{Ran, RanOn, Session, Why};
pub use time::{KeptTime, Stretch, Went};

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
    /// The observations its runs kept as each finished, in the order they were kept (module [`observations`]).
    pub observed: Vec<Observed>,
    /// What the calls of each try of an agent's round to the model came to, where its carrier said, in order.
    pub measured: Vec<Measured>,
    /// Each keep-warm ping of a line's session, in order.
    pub pinged: Vec<Pinged>,
    /// Each fork of a round's session asked a question, in order.
    pub forked: Vec<Forked>,
    /// Each pull request the host of the work's repository found the work checked through, in order, where it is
    /// another than the one before: the latest is the work's.
    pub pulls: Vec<Pulled>,
    /// Why the work's item was cancelled, by the place of each cancel, as whoever cancelled it said; none for a cancel
    /// where nothing was said, as one kept before a cancel said why.
    pub why_cancelled: BTreeMap<Seq, String>,
    /// Why the work's item was reopened, by the place of each reopen, as whoever reopened it said.
    pub why_reopened: BTreeMap<Seq, String>,
    /// Each version of its flow the work was put on, in order (module [`flows`]).
    pub flows: Vec<OnFlow>,
    /// How whoever holds the work read what it recorded before each of its entries in `flows`, in order: no record, and
    /// none of a work as the store gives it.
    pub readings: Vec<Read>,
    /// What its time went to, kept each time it ended, in order (module [`time`]).
    pub kept_time: Vec<KeptTime>,
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
            .chain(r.hurried.iter().map(|e| e.at))
            .chain(r.written.iter().map(|e| e.at))
            .chain(r.questions.iter().map(|e| e.at))
            .chain(r.answers.iter().map(|e| e.at))
            .chain(r.let_go.iter().map(|e| e.at))
            .chain(r.pauses.iter().map(|e| e.at))
            .chain(r.cancels.iter().copied())
            .chain(r.reopens.iter().copied())
            .chain(r.releases.iter().copied())
            .chain(self.sessions.iter().map(|s| s.at))
            .chain(self.filled.iter().map(|f| f.at))
            .chain(self.ran.iter().map(|r| r.at))
            .chain(self.measured.iter().map(|m| m.at))
            .chain(self.pinged.iter().map(|p| p.at))
            .chain(self.forked.iter().map(|f| f.at))
            .chain(self.pulls.iter().map(|p| p.at))
            .chain(self.flows.iter().map(|f| f.at))
            .chain(self.kept_time.iter().map(|k| k.at));
        Seq(places.map(|at| at.0).max().unwrap_or(0) + 1)
    }

    /// When the entry at this place was added: the time of the thing that happened it belongs to. A place after the
    /// last entry gives the time of the last thing that happened, and a place before the first gives none.
    pub fn time_of(&self, place: Seq) -> Option<Time> {
        self.moments.iter().rev().find(|m| m.from <= place).map(|m| m.at)
    }

    /// What the work's calls beside its rounds took, in the order of what happened to it: each keep-warm ping and each
    /// fork. What a work spent in all is its rounds' and these.
    pub fn spent_beside_rounds(&self) -> Vec<SpentBeside> {
        let pings = self.pinged.iter().map(|p| SpentBeside {
            at: p.at,
            kind: BesideKind::Ping,
            spent: p.spent,
        });
        let forks = self.forked.iter().map(|f| SpentBeside {
            at: f.at,
            kind: BesideKind::Fork,
            spent: Spent {
                usd_micros: f.usd_micros,
                input_tokens: f.input_tokens,
                cached_tokens: f.cached_tokens,
                output_tokens: f.output_tokens,
                ..Spent::default()
            },
        });
        let mut all: Vec<SpentBeside> = pings.chain(forks).collect();
        all.sort_by_key(|b| b.at);
        all
    }

    /// A name no run and no call of the work has yet: one after the latest. Whoever starts a run or makes a call asks
    /// for it, and names the run or the call so.
    pub fn name(&self) -> u64 {
        let r = &self.records;
        let names = (r.started.iter().map(|s| s.run.0)).chain(r.called.iter().map(|c| c.call.0));
        names.max().unwrap_or(0) + 1
    }
}

/// The digest of a text that tells it apart from another and keeps nothing of it: its SHA-1, in hexadecimal (principle
/// `stored-content-is-kept-small`).
pub fn digest(text: &str) -> String {
    sha1_smol::Sha1::from(text).digest().to_string()
}

/// A pull request the host of the work's repository found the work checked through, opening it where the work had
/// none, at a place of its own in the order of what happened to the work: its number on the host, and its address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pulled {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub number: u64,
    pub url: String,
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
    /// The results of calls: of those the earlier runs of the cell asked for, given to the first round of the run
    /// entered for them; or, in works kept before a return that asks ended its run, of those the round before asked
    /// for.
    Results(Vec<CallId>),
    /// Why what the round before handed in was sent back.
    SentBack(Vec<Unmet>),
}

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Back {
    /// Calls were asked for, by a return of the form before a return said what it dealt with. The calls made for them
    /// are in the records: those of the run made after this round ended and before its next began.
    Asked(Vec<Ask>),
    /// These blocks were handed in, by a return of the form before a return said what it dealt with: it dealt with
    /// everything that ran its round.
    HandedIn(Vec<Handed>),
    /// A return: which of what ran the round it dealt with, in the order it named them, the blocks it writes, and the
    /// calls it asks for (design document `data-and-context`, 9, 27, 30). Taken, the calls are made as its run
    /// finishes, before the finish. What was found wrong with it as it was read, which only its reading could see, is
    /// kept with it, so that the run is decided again from the records alone; and so is what it says beside its work.
    /// Returns kept before a return was read so have neither, and read as having none. Beside it is what the engine
    /// itself asks the owner from what it found of the work as the round ended, which no word of the agent's is;
    /// returns kept before the engine asked anything have none. And what the carrier read of the work's branch as the
    /// round ended, which the run writes as it finishes, as what the round committed; returns kept before the branch
    /// was read so have none.
    Returned {
        dealt_with: Vec<BlockId>,
        handed: Vec<Handed>,
        asks: Vec<Ask>,
        #[serde(default)]
        found: Vec<Unmet>,
        #[serde(default)]
        said: Said,
        #[serde(default)]
        engine: Vec<Question>,
        /// A block a source outside the work writes, a new version of which what it writes waits for, where the
        /// engine's own act says so; returns kept before an act said it have none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        waits_for: Option<BlockId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        read: Vec<Handed>,
    },
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

/// What a return says beside the blocks it hands in: its questions, which it writes in the block the engine keeps as
/// them and which are data of their own once its run finishes (design document `data-and-context`, 26), and its
/// observations. Its answer to what the owner said to its cell is a block it hands in, its cell's own words; a return
/// kept before that held it here, and is read without it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    /// What it asks the owner, each question by its name, in the order it wrote them.
    #[serde(default)]
    pub questions: Vec<Question>,
    /// What its agent came across beyond the work, each a text.
    #[serde(default)]
    pub observations: Vec<String>,
}

impl Said {
    /// Whether a question it asks waits for the owner's answer: the work stops at its cell until the answer comes.
    pub fn waits(&self) -> bool {
        self.waiting() > 0
    }

    /// How many of the questions it asks wait for the owner's answer.
    pub fn waiting(&self) -> usize {
        (self.questions.iter())
            .filter(|q| matches!(q, Question::Asked { waits: true, .. }))
            .count()
    }
}

impl Question {
    /// Its name.
    pub fn name(&self) -> &str {
        match self {
            Question::Asked { name, .. } | Question::Withdrawn { name, .. } => name,
        }
    }
}

impl Work {
    /// What the version of a question asks and its options, or why it was withdrawn: as the round that brought it
    /// returned it, the last of its run to end before the version was written, among what the agent said or what the
    /// engine asked beside it. None where that round's return holds no question of the name.
    pub fn question(&self, version: &decide::Questioned) -> Option<&Question> {
        let r = &self.records;
        let round = (r.returned.iter())
            .filter(|e| e.run == version.run && e.at < version.at)
            .count();
        match self.back.get(&(version.run, round))? {
            Back::Returned { said, engine, .. } => {
                let of = if version.engine { engine } else { &said.questions };
                of.iter().rev().find(|q| q.name() == version.name)
            }
            _ => None,
        }
    }
}

/// A question a return asks the owner, or withdraws, under its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Question {
    /// The question asked, or asked anew: whether its cell waits for the answer, whether the option taken goes into
    /// the design tree (`tree`), the option it recommends, counted from one, where it recommends one, what it asks, and
    /// its options; and, of a question of the engine's, what it found that the question is of, told apart from what any
    /// other version is of, and why the work's agents said each part of it is needed, by the part's name: of a change
    /// of the paths the project protects, each file's reason by its path, which a later version asks with again where
    /// the file is as it was. `tree` is none where the question is not marked: one the engine asks, and one asked
    /// before questions said it, read so.
    Asked {
        name: String,
        waits: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tree: Option<bool>,
        recommends: Option<u64>,
        ask: String,
        options: Vec<Way>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        about: Option<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        reasons: BTreeMap<String, String>,
    },
    /// The question withdrawn, for this reason.
    Withdrawn { name: String, why: String },
}

/// An option of a question, one of the ways that each would do: what the work does by it, and what follows from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Way {
    pub does: String,
    pub follows: String,
}

/// A block as a run hands it in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handed {
    pub block: BlockId,
    pub content: String,
    /// Whether the change matters to the cells the block runs: one that does not is handed in as changing nothing in
    /// meaning, and runs no cell.
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
    /// A question for a person. No return that asks one is taken (the engine fails its round); the variant stays so
    /// that a call recorded as one is read as it was recorded.
    Person { who: Who, question: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(usd_micros: u64) -> Spent {
        Spent {
            usd_micros,
            input_tokens: 1,
            cached_tokens: 2,
            output_tokens: 3,
            context_tokens: Some(9),
            compactions: 1,
        }
    }

    // What a work spent beside its rounds is its keep-warm pings' and its forks', each at its place, in the order of
    // what happened, whichever table keeps it; a fork's has only what a fork keeps. Microseconds.
    #[test]
    fn what_a_work_spent_beside_its_rounds_gives_pings_and_forks_in_the_order_of_their_places() {
        let mut work = Work::default();
        let ping = |at, usd| Pinged {
            at: Seq(at),
            line: "engineer".to_string(),
            session: 0,
            spent: cost(usd),
            failed: None,
        };
        let fork = |at, usd| Forked {
            at: Seq(at),
            run: RunId(1),
            round: 1,
            usd_micros: usd,
            input_tokens: 1,
            cached_tokens: 2,
            output_tokens: 3,
        };
        work.pinged = vec![ping(2, 10), ping(6, 30)];
        work.forked = vec![fork(4, 20), fork(8, 40)];
        let beside = work.spent_beside_rounds();
        let seen: Vec<(u64, BesideKind, u64)> = beside.iter().map(|b| (b.at.0, b.kind, b.spent.usd_micros)).collect();
        assert_eq!(
            seen,
            [
                (2, BesideKind::Ping, 10),
                (4, BesideKind::Fork, 20),
                (6, BesideKind::Ping, 30),
                (8, BesideKind::Fork, 40)
            ]
        );
        // A ping keeps what its session held and how often it compacted; a fork's spend has none of them.
        assert_eq!(beside[0].spent, cost(10));
        assert_eq!(
            beside[1].spent,
            Spent {
                context_tokens: None,
                compactions: 0,
                ..cost(20)
            }
        );
        assert_eq!(work.tick(), Seq(9), "a fork's place is one the next entry follows");
    }
}
