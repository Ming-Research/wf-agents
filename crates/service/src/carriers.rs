//! What carries out what the engine gives to be done, as the loop reaches it.
//!
//! A carrier is given what it needs to do its thing without reaching into the driver: the work's number, its flow,
//! the cell's specification, the run, the round and what it is given, the item the work is done for, and the work as
//! it is kept when the todo is handed out. It does the thing on its own time, and says once what came of it through
//! the reply it was handed with it, which takes that into the loop's channel. A carrier gives no answer to the loop
//! otherwise: what it does is known only by what it says came of it.
//!
//! What was to be done and is not known as done is handed out again when the process starts again, with the same
//! work, run, round and call: a carrier recognises by those what it began before the stop (principle
//! `effects-known-as-done`), and answers it once.
//!
//! A round a carrier cannot carry out it says failed, with why, and with the kind of cause where it tells one apart: the
//! engine handles the failure as the kind says, pausing the work with the reason where that is the kind's handling, and
//! the run's next round is given the same as the failed one, a new round that the carrier carries out anew. The
//! carrier keeps nothing of a failed round, and tries nothing again itself: when a round is tried again is the engine's
//! to say, as its records show, and whatever made the round fail is shown on the page.

use std::sync::Arc;

use decide::run::Failure;
use decide::{CallId, Pauser, RunId};
use engine::{Came, Event};
use flow::{CellSpec, Compiled, Kind};
use store::{Item, WorkNumber};
use tokio::sync::mpsc;
use work::{Given, Spent, Time, Work};

use crate::Message;

/// The result a withdrawn call has: given by the loop to a person's call, and by what runs a tool to a tool's call it
/// stopped.
pub const WITHDRAWN: &str = "Withdrawn: its run was abandoned before it had its result.";

/// What carries out the rounds of cells of one kind: an agent's rounds, or the engine's own acts.
///
/// Both methods are called inside the loop, between two things it takes: each returns at once, and does its work
/// elsewhere, on a task or a thread of its own, so that the loop is held up by no round.
pub trait Rounds: Send + Sync {
    /// Gives the round: an agent is given it in the session of the cell's line, or the act is done. The round ends
    /// when the reply says what came back and what it took, or that it failed and why. Given again after the process started again, it is the
    /// same round, to be gone on with where what was done of it is known, and done again where it is not.
    fn give(&self, round: Round, reply: RoundReply);

    /// Stops the round, which is under way: its work was cancelled. The round ends as it always does, by a reply
    /// saying what it took; what came back is dropped. A round has one end: where the reply it was given with is
    /// still held, that one is used and this one dropped, and where the round has ended already this one is dropped.
    fn stop(&self, round: Round, reply: RoundReply);

    /// What the round of this number of the run was told and what came back of it, each try in order, as far as
    /// this carrier keeps them; no try where it keeps nothing, as by default. Unlike the others, this is called off
    /// the loop, on a thread that may wait on the disk; an error says why what was kept could not be read.
    fn told(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        let _ = (work, run, round);
        Ok(Told::default())
    }
}

/// What a round was told and what came back of it, as its carrier kept them: each try of it, in order. A round is
/// tried more than once where its carrier ran its program again for it. What is read of a round is bounded, and each
/// cut is said: a long message is cut, and tries past what is read of a round are left out and counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Told {
    pub tries: Vec<Try>,
    /// How many later tries were left out, those before them being as much as is read of a round.
    pub left_out: usize,
}

/// One try of a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Try {
    /// The message it was told, as it was written, up to where it was cut.
    pub message: String,
    /// The message's whole length in bytes, where it was cut before its end.
    pub cut: Option<u64>,
    pub back: Back,
}

/// What came back of a try.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Back {
    /// Nothing that was kept: the try is under way, or it was stopped, never got as far as its program, or its program
    /// printed no result.
    Nothing,
    Returned(Returned),
    /// The program said the try failed, for this reason.
    Failed(String),
    /// What was kept of it could not be read, for this reason.
    Unreadable(String),
}

/// What came back of a try, as far as the page is given it: never the whole of what the program printed, which holds
/// what is not for the page, such as ids of sessions and of credentials.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Returned {
    /// The agent's closing words; none where they are the structured return itself.
    pub text: Option<String>,
    /// The structured return, as pretty-printed JSON.
    pub structured: Option<String>,
    pub turns: Option<u64>,
    pub duration_ms: Option<u64>,
    /// The models it was answered by, by name.
    pub models: Vec<String>,
}

/// What runs the tools that agents ask for.
///
/// Both methods are called inside the loop, between two things it takes: each returns at once, and does its work
/// elsewhere, on a task or a thread of its own, so that the loop is held up by no tool.
pub trait Tools: Send + Sync {
    /// Runs the tool. The call has its result when the reply gives it.
    fn call(&self, call: ToolCall, reply: CallReply);

    /// Withdraws the call, whose run was abandoned: the tool is stopped where it runs, and the call has
    /// [`WITHDRAWN`] for its result, or the result that came before it could be stopped. A call has one result:
    /// where the reply it was made with is still held, that one is used and this one dropped, and where the call's
    /// result was sent already this one is dropped and nothing more is sent.
    fn withdraw(&self, call: ToolCall, reply: CallReply);
}

/// What looks at the network while the agents' rounds wait for it to be reached: whether the agents' service can be
/// reached from here, as far as a look that costs no round and writes nothing in a session can tell.
///
/// It is called off the loop, on a thread that may wait on the network for a few seconds.
pub trait Network: Send + Sync {
    /// Whether the network reaches the agents' service now.
    fn reached(&self) -> bool;
}

/// What carries out each kind of todo.
#[derive(Clone)]
pub struct Carriers {
    /// The rounds of the cells an agent carries out.
    pub agents: Arc<dyn Rounds>,
    /// The rounds of the cells that are the engine's own acts.
    pub acts: Arc<dyn Rounds>,
    pub tools: Arc<dyn Tools>,
    pub network: Arc<dyn Network>,
}

impl Carriers {
    /// What carries out the rounds of cells of this kind.
    pub fn rounds(&self, kind: Kind) -> &Arc<dyn Rounds> {
        match kind {
            Kind::Agent => &self.agents,
            Kind::Engine => &self.acts,
        }
    }

    /// What the round was told and returned, as the carrier of its cell's kind kept them; for a cell of no kind
    /// known, one the flow no longer has, as the first carrier that kept anything of it. It waits on the disk.
    pub fn told(&self, kind: Option<Kind>, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        let carriers = match kind {
            Some(kind) => vec![self.rounds(kind)],
            None => vec![&self.agents, &self.acts],
        };
        let mut told = Told::default();
        for rounds in carriers {
            told = rounds.told(work, run, round)?;
            if !told.tries.is_empty() {
                break;
            }
        }
        Ok(told)
    }
}

/// A round of a run, to give or to stop.
#[derive(Clone, Debug)]
pub struct Round {
    pub work: WorkNumber,
    /// The flow the work runs on.
    pub flow: Arc<Compiled>,
    pub cell: CellSpec,
    pub run: RunId,
    /// Which round of the run it is, the first being one.
    pub number: usize,
    pub given: Given,
    /// The item the work is done for: its title and the items it depends on, which do not change once it is added.
    pub item: Item,
    /// The work as it was kept when the round was handed out.
    pub kept: Arc<Work>,
}

/// A call of a run that a tool answers, to make or to withdraw.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub work: WorkNumber,
    pub flow: Arc<Compiled>,
    pub run: RunId,
    pub call: CallId,
    /// The tool.
    pub name: String,
    /// What it is run on.
    pub input: String,
    /// The item the work is done for, as a round is given it.
    pub item: Item,
    /// The work as it was kept when the call was handed out.
    pub kept: Arc<Work>,
}

/// The way to say once that a round ended, or that it failed.
#[derive(Debug)]
pub struct RoundReply {
    work: WorkNumber,
    run: RunId,
    to: mpsc::UnboundedSender<Message>,
}

/// The way to give a call its result, once.
#[derive(Debug)]
pub struct CallReply {
    work: WorkNumber,
    call: CallId,
    to: mpsc::UnboundedSender<Message>,
}

impl RoundReply {
    pub(crate) fn new(work: WorkNumber, run: RunId, to: mpsc::UnboundedSender<Message>) -> Self {
        RoundReply { work, run, to }
    }

    /// The round ended: this came back, and it took this.
    pub fn ended(self, came: Came, spent: Spent) {
        let event = Event::Returned {
            run: self.run,
            came,
            spent,
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }

    /// The round ended as [`RoundReply::ended`] says, and its work is to wait for the owner, for this reason, which
    /// the owner reads: the carrier found that the work should not go on before the owner has looked, as when it has
    /// spent more than it may. The engine's pause and then the end are sent down the loop's one channel in that order,
    /// so that the loop takes the pause while the round is still under way: what came back is taken as from any
    /// round, and no next round begins. The owner lets the work on again as from any pause.
    pub fn ended_and_paused(self, came: Came, spent: Spent, why: String) {
        let paused = Event::Paused {
            by: Pauser::Engine,
            why: Some(why),
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: paused,
        });
        self.ended(came, spent);
    }

    /// The round failed, for this reason, having taken this: nothing came back, for a cause no kind tells apart. The
    /// engine pauses the work with the reason as it takes this; once the owner lets the work on, the run's next round
    /// is given what this one was.
    pub fn failed(self, why: String, spent: Spent) {
        self.failed_as(Failure::Other, None, why, spent);
    }

    /// The round failed as [`RoundReply::failed_as`] says, and its work is to wait for the owner, for this reason, as
    /// when the round took it over its budget: the pause is sent first, as [`RoundReply::ended_and_paused`] sends it.
    pub fn failed_and_paused(self, failure: Failure, resets: Option<Time>, why: String, spent: Spent, pause: String) {
        let paused = Event::Paused {
            by: Pauser::Engine,
            why: Some(pause),
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: paused,
        });
        self.failed_as(failure, resets, why, spent);
    }

    /// The round failed, for this reason, having taken this, for a cause of this kind, as the carrier told it apart;
    /// `resets` says when the account's usage limit resets, where that is the cause and its report says when. The
    /// engine handles the failure as its kind says.
    pub fn failed_as(self, failure: Failure, resets: Option<Time>, why: String, spent: Spent) {
        let event = Event::Failed {
            run: self.run,
            why,
            failure,
            resets,
            spent,
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

impl CallReply {
    pub(crate) fn new(work: WorkNumber, call: CallId, to: mpsc::UnboundedSender<Message>) -> Self {
        CallReply { work, call, to }
    }

    /// The call has this result.
    pub fn done(self, result: String) {
        let event = Event::Done {
            call: self.call,
            result,
        };
        // Where the loop has ended, the process is stopping: the call is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rounds that keep, of every round, one try with this message, or nothing.
    struct Keeps(Option<&'static str>);

    impl Rounds for Keeps {
        fn give(&self, _: Round, _: RoundReply) {}

        fn stop(&self, _: Round, _: RoundReply) {}

        fn told(&self, _: WorkNumber, _: RunId, _: usize) -> Result<Told, String> {
            let tries = (self.0.iter())
                .map(|message| Try {
                    message: message.to_string(),
                    cut: None,
                    back: Back::Nothing,
                })
                .collect();
            Ok(Told { tries, left_out: 0 })
        }
    }

    struct NoTools;

    impl Tools for NoTools {
        fn call(&self, _: ToolCall, _: CallReply) {}

        fn withdraw(&self, _: ToolCall, _: CallReply) {}
    }

    struct Reached;

    impl Network for Reached {
        fn reached(&self) -> bool {
            true
        }
    }

    fn carriers(agents: Option<&'static str>, acts: Option<&'static str>) -> Carriers {
        Carriers {
            agents: Arc::new(Keeps(agents)),
            acts: Arc::new(Keeps(acts)),
            tools: Arc::new(NoTools),
            network: Arc::new(Reached),
        }
    }

    fn messages(told: Told) -> Vec<String> {
        told.tries.into_iter().map(|t| t.message).collect()
    }

    #[test]
    fn a_round_is_told_by_the_carrier_of_its_cells_kind() {
        let carriers = carriers(Some("an agent's"), Some("an act's"));
        let told = |kind| messages(carriers.told(Some(kind), WorkNumber(1), RunId(1), 1).unwrap());
        assert_eq!(told(Kind::Agent), ["an agent's"]);
        assert_eq!(told(Kind::Engine), ["an act's"]);
    }

    #[test]
    fn a_round_of_a_cell_the_flow_no_longer_has_is_told_by_the_first_carrier_that_kept_it() {
        let told = |carriers: Carriers| messages(carriers.told(None, WorkNumber(1), RunId(1), 1).unwrap());
        assert_eq!(told(carriers(Some("an agent's"), None)), ["an agent's"]);
        assert_eq!(told(carriers(None, Some("an act's"))), ["an act's"]);
        assert!(told(carriers(None, None)).is_empty());
    }
}
