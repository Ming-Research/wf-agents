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
use decide::{BlockId, CallId, End, Outsider, Pauser, RunId};
use driver::Policy;
use engine::{Came, Event};
use flow::{CellSpec, Compiled, Kind};
use store::{Item, WorkNumber};
use tokio::sync::mpsc;
use work::{Fill, Given, RanOn, Spent, Time, Why, Work};

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

    /// The system prompt the line's session of this number, the line's first being none, began with, as this carrier
    /// kept it for the work; none where it kept none, as by default and for rounds carried out in no session. It is
    /// called off the loop, as [`Rounds::told`] is.
    fn system(&self, work: WorkNumber, line: &str, session: u32) -> Result<Option<String>, String> {
        let _ = (work, line, session);
        Ok(None)
    }

    /// What the agent of the round of this number of the run was last seen doing, as this carrier keeps it as it
    /// comes: the last few things its latest try did, the latest last; nothing where it keeps none, as by default. It
    /// is called off the loop, as [`Rounds::told`] is.
    fn doing(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Vec<Doing>, String> {
        let _ = (work, run, round);
        Ok(Vec::new())
    }

    /// When a line begins a new session, for rounds carried out in sessions: what they run on, and how far a session
    /// may grow. None, as by default, for rounds carried out in no session, whose lines the loop begins no session
    /// for itself.
    fn policy(&self) -> Option<Policy> {
        None
    }

    /// Lets go of what the rounds of this kind made for the work outside the engine, the work having ended and nothing
    /// of it being under way; the reply says once that it is let go. Given again after the process started again, it is
    /// the same letting go, and what was let go already is found let go. Rounds that make nothing outside the engine
    /// let go of nothing, as by default.
    fn release(&self, release: Release, reply: ReleaseReply) {
        let _ = release;
        reply.released();
    }
}

/// A work that has ended, whose rounds' carriers let go of what they made for it outside the engine.
#[derive(Clone, Debug)]
pub struct Release {
    pub work: WorkNumber,
    /// How it ended.
    pub end: End,
    /// The flow the work runs on.
    pub flow: Arc<Compiled>,
    /// The item the work is done for.
    pub item: Item,
    /// The work as it was kept when its release was handed out.
    pub kept: Arc<Work>,
}

/// The way to say once that what a work held is let go.
#[derive(Debug)]
pub struct ReleaseReply {
    work: WorkNumber,
    to: mpsc::UnboundedSender<Message>,
}

impl ReleaseReply {
    /// The reply that says what came of it down this channel, as the loop makes it; whoever tests a carrier makes one
    /// to read what the carrier says.
    pub fn new(work: WorkNumber, to: mpsc::UnboundedSender<Message>) -> Self {
        ReleaseReply { work, to }
    }

    /// What the work held could not be let go, for this reason.
    pub fn failed(self, why: String) {
        // Where the loop has ended, the process is stopping: the release is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: Event::NotReleased { why },
        });
    }

    /// What the work held is let go.
    pub fn released(self) {
        // Where the loop has ended, the process is stopping: the release is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: Event::Released,
        });
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
    /// What it ran on, as the work's records say its carrier chose it as the try began, or, for a try that began
    /// before tries were recorded, as its carrier kept it beside the try; none where neither says, as of a try kept
    /// before either, or of an act of the engine's own.
    pub ran: Option<RanOn>,
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
    /// The tokens it wrote to the prompt cache, which the records count among those not read from it.
    pub cache_creation_input_tokens: Option<u64>,
}

/// A thing an agent was seen doing during its round, as its program said it as it went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Doing {
    /// It used a tool by this name, on this where the tool's input names what: a command, a path, a pattern.
    Tool { name: String, on: Option<String> },
    /// It said this; the start of it where it is long.
    Said(String),
    /// Its session was being compacted.
    Compacting,
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
    /// How many rounds of agents may go on at once: the engine gives the agents no more than this, and the others wait
    /// for a slot in the engine's order. The engine's own acts take none.
    pub slots: usize,
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

    /// The system prompt the line's session of this number, the line's first being none, began with, as the agents'
    /// carrier kept it. It waits on the disk.
    pub fn system(&self, work: WorkNumber, line: &str, session: u32) -> Result<Option<String>, String> {
        self.agents.system(work, line, session)
    }

    /// What the agent of the round was last seen doing, as the carrier of its cell's kind keeps it, or, for a cell of
    /// no kind known, as the first carrier that keeps anything of it. It waits on the disk.
    pub fn doing(&self, kind: Option<Kind>, work: WorkNumber, run: RunId, round: usize) -> Result<Vec<Doing>, String> {
        let carriers = match kind {
            Some(kind) => vec![self.rounds(kind)],
            None => vec![&self.agents, &self.acts],
        };
        let mut doing = Vec::new();
        for rounds in carriers {
            doing = rounds.doing(work, run, round)?;
            if !doing.is_empty() {
                break;
            }
        }
        Ok(doing)
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
    /// The reply that says what came of it down this channel, as the loop makes it; whoever tests a carrier makes one
    /// to read what the carrier says.
    pub fn new(work: WorkNumber, run: RunId, to: mpsc::UnboundedSender<Message>) -> Self {
        RoundReply { work, run, to }
    }

    /// The way to say, while the round is under way, what each of its tries runs on and which session of its line it
    /// began for a reason of its own.
    pub fn teller(&self) -> Teller {
        Teller {
            work: self.work,
            run: self.run,
            to: self.to.clone(),
        }
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

/// The way to say what happens while a round is under way, as often as it happens: each is taken while the round is
/// still under way, before its end.
#[derive(Clone, Debug)]
pub struct Teller {
    work: WorkNumber,
    run: RunId,
    to: mpsc::UnboundedSender<Message>,
}

impl Teller {
    /// The try of this number of the round, the first being one, begins on this.
    pub fn ran_on(&self, attempt: usize, on: RanOn) {
        self.tell(Event::Ran {
            run: self.run,
            attempt,
            on,
        });
    }

    /// The message of the try of this number of the round, the first being one, fills this into its session: said
    /// after the try's beginning and before its message is given, so that what a session holds is in the records.
    pub fn filled(&self, attempt: usize, fills: Vec<Fill>) {
        self.tell(Event::Filled {
            run: self.run,
            attempt,
            fills,
        });
    }

    /// The line's session was reopened during the round, for this reason: the agents' program had lost the one the
    /// round was to go on in, or compacted it.
    pub fn session_began(&self, line: String, why: Why) {
        self.tell(Event::Anew { line, why });
    }

    /// The source outside the work that writes the block says this of it now: the carrier read it of what the round
    /// leaves, as the design tree on the work's branch after a round that committed to it. It is written as a new
    /// version of the block, while the round is under way.
    pub fn outside_wrote(&self, block: BlockId, content: String) {
        self.tell(Event::Wrote {
            block,
            by: Outsider::Outside,
            content,
            matters: true,
            value: None,
        });
    }

    /// As [`Teller::outside_wrote`], for the round: the version is put into it, so that the round is told of it and its
    /// run consumes it. The carrier does it for what it did before its program began, as taking main in.
    pub fn outside_wrote_for_round(&self, block: BlockId, content: String) {
        self.tell(Event::WroteForRound {
            run: self.run,
            block,
            content,
        });
    }

    /// The service of the agents answered the round with a message that is no error: it serves the round, which a wait
    /// every work shares for it may be over by.
    pub fn answered(&self) {
        let _ = self.to.send(Message::Answered {
            work: self.work,
            run: self.run,
        });
    }

    fn tell(&self, event: Event) {
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

impl CallReply {
    /// The reply that says what came of it down this channel, as the loop makes it; whoever tests a carrier makes one
    /// to read what the carrier says.
    pub fn new(work: WorkNumber, call: CallId, to: mpsc::UnboundedSender<Message>) -> Self {
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
                    ran: None,
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
            slots: 1,
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
