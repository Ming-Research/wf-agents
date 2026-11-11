//! What happens to a work, and what is to be done for it.

use decide::run::Unmet;
use decide::{BlockId, CallId, CellId, Outsider, Pauser, RunId};

use work::{Ask, Call, Given, Handed, RanOn, Spent, Why};

/// One thing that happened to a work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Nothing new is recorded: the work is looked at again. A new work, or a changed flow.
    Look,
    /// A person or the outside wrote a block.
    Wrote {
        block: BlockId,
        by: Outsider,
        content: String,
        /// Whether what the outside wrote changes the block's meaning; not read for a person's prose or a choice.
        matters: bool,
        value: Option<String>,
    },
    /// The owner let the cell go on at its limit.
    LetGo { cell: CellId },
    /// The work was paused: by the owner, who gives no reason, or by the engine itself, for this reason, as when it
    /// spent more than it may. A round that fails pauses its work by [`Event::Failed`] instead.
    Paused { by: Pauser, why: Option<String> },
    /// The work was let on again.
    Resumed,
    /// The work's item was cancelled.
    Cancelled,
    /// The round of the run that was under way ended, having taken this.
    Returned { run: RunId, came: Came, spent: Spent },
    /// The round of the run that was under way failed, for this reason, having taken this: nothing came back. Where
    /// the run is open, the engine pauses the work with the reason as it takes this, and once the work is let on the
    /// run's next round is given what the failed one was given.
    Failed { run: RunId, why: String, spent: Spent },
    /// The call has its result.
    Done { call: CallId, result: String },
    /// A new session of the line began, or is to begin with the line's next round, for this reason.
    Anew { line: String, why: Why },
    /// A try of the round of the run that is under way began, on this.
    Ran { run: RunId, on: RanOn },
}

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Came {
    /// The agent asks for these.
    Asks(Vec<Ask>),
    /// The agent, or the engine's act, hands in these blocks.
    HandsIn(Vec<Handed>),
}

/// What is to be done for a work outside the engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// This round of the run is given to the cell's agent, or acted out where the cell is the engine's own.
    Round {
        run: RunId,
        cell: CellId,
        /// Which round of the run it is, the first being one.
        number: usize,
        given: Given,
    },
    /// This call is made.
    Call { call: CallId, run: RunId, what: Call },
    /// This call is withdrawn: its run was abandoned. It is to be done until the call is said to have its result,
    /// which is then that it was withdrawn, or whatever came back before it could be.
    Withdraw { call: CallId },
    /// The round of this run that is under way is stopped: the run was abandoned with its work cancelled. It is to
    /// be done until the round is said to have ended; what comes back from it is dropped.
    Stop { run: RunId },
}

/// Why what happened was not taken: it cannot have happened to this work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The flow has no such block.
    NoSuchBlock(BlockId),
    /// The flow does not let this writer write the block.
    NotTheirs { block: BlockId, by: Outsider },
    /// The block was written with a value it cannot have, or without the value it must have.
    Unmet(Unmet),
    /// The flow has no such cell.
    NoSuchCell(CellId),
    /// No round of the run is under way.
    NoRoundUnderWay(RunId),
    /// The work has no such call.
    NoSuchCall(CallId),
    /// The call has its result already.
    AlreadyDone(CallId),
    /// The flow has no cell on such a line.
    NoSuchLine(String),
}
