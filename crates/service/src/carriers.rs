//! What carries out what the engine gives to be done, as the loop reaches it.
//!
//! A carrier is given what it needs to do its thing without reaching into the driver: the work's number, its flow,
//! the cell's specification, the run, the round and what it is given, and the work as it is kept when the todo is
//! handed out. It does the thing on its own time, and says once what came of it through the reply it was handed with
//! it, which takes that into the loop's channel. A carrier gives no answer to the loop otherwise: what it does is
//! known only by what it says came of it.
//!
//! What was to be done and is not known as done is handed out again when the process starts again, with the same
//! work, run, round and call: a carrier recognises by those what it began before the stop (principle
//! `effects-known-as-done`), and answers it once.

use std::sync::Arc;

use decide::{CallId, RunId};
use engine::{Came, Event};
use flow::{CellSpec, Compiled};
use store::WorkNumber;
use tokio::sync::mpsc;
use work::{Given, Spent, Work};

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
    /// when the reply says what came back and what it took. Given again after the process started again, it is the
    /// same round, to be gone on with where what was done of it is known, and done again where it is not.
    fn give(&self, round: Round, reply: RoundReply);

    /// Stops the round, which is under way: its work was cancelled. The round ends as it always does, by a reply
    /// saying what it took; what came back is dropped. A round has one end: where the reply it was given with is
    /// still held, that one is used and this one dropped, and where the round has ended already this one is dropped.
    fn stop(&self, round: Round, reply: RoundReply);
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

/// What carries out each kind of todo.
#[derive(Clone)]
pub struct Carriers {
    /// The rounds of the cells an agent carries out.
    pub agents: Arc<dyn Rounds>,
    /// The rounds of the cells that are the engine's own acts.
    pub acts: Arc<dyn Rounds>,
    pub tools: Arc<dyn Tools>,
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
    /// The work as it was kept when the call was handed out.
    pub kept: Arc<Work>,
}

/// The way to say once that a round ended.
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
