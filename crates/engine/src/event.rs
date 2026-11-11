//! What happens to a work, and what is to be done for it.

use decide::run::{Failure, Unmet};
use decide::{BlockId, CallId, CellId, End, Outsider, Pauser, RunId};

use work::{Ask, Call, Given, Handed, Spent, Time};

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
    /// The round of the run that was under way failed, for this reason, having taken this: nothing came back. The
    /// cause is of the kind `failure`, as whatever carried the round out told it apart, and `resets` says when the
    /// account's usage limit resets, where that is the cause and its report says when. Where the run is open, the
    /// engine handles the failure as its kind says (crate `decide`, module `run`): where that is to pause the work, it
    /// pauses it with the reason as it takes this; the run's next round is given what the failed one was given.
    Failed {
        run: RunId,
        why: String,
        failure: Failure,
        resets: Option<Time>,
        spent: Spent,
    },
    /// The call has its result.
    Done { call: CallId, result: String },
    /// What the work held outside the engine was let go, as [`Effect::Release`] asked.
    Released,
    /// What the work held outside the engine could not be let go, for this reason, as whatever carried out
    /// [`Effect::Release`] says.
    NotReleased { why: String },
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
    /// The work has ended so, and nothing of it is under way: what it holds outside the engine is let go (its
    /// worktree, and its pull request where it did not merge; its branch stays). It is to be done until the work is
    /// said to have released it.
    Release { end: End },
}

/// Which rounds may begin, of this work as of every other: a round that failed for a cause every work shares, such as
/// the account's usage limit, holds them, as whatever drives the engine works out from the records of every work. It
/// holds back nothing of a run but the beginning of its rounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gate {
    /// Every round may begin.
    #[default]
    Open,
    /// No round of an agent begins; the engine's own acts do.
    Agents,
    /// No round begins, the engine's own acts' included: they may reach the network, which cannot be reached.
    Every,
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
    /// The work has not ended, or something of it is still under way: it has nothing to let go yet.
    NotToRelease,
}
