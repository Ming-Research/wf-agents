//! What a run does next, from what it has done so far.
//!
//! A run is made of rounds. At the end of a round its agent, or the engine's own act, either asks for calls, which
//! the engine then makes and whose results the next round is given, or hands in the blocks it wrote. What came back
//! and does not meet what the flow requires is sent back: the next round is given why. A run finishes when what was
//! handed in meets everything, and writes its blocks then. How often something is sent back and how many calls are
//! made each has a limit, at which the owner lets the run go on or cancels the work.
//!
//! The walk says which cell runs and which run goes on; this says what going on is.

use std::collections::BTreeMap;

use crate::flow::BlockId;
use crate::records::{CallId, Write};

/// What a run has done so far: its rounds, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    pub rounds: Vec<Round>,
    /// How many of the rounds had ended when the owner last let the run go on at a limit; none at first. The counts
    /// that have a limit start anew from there.
    pub let_go: usize,
}

/// A round of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    /// What came back at the end of the round; `None` while the round is under way.
    pub returned: Option<Return>,
}

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Return {
    /// Calls were asked for: a tool the engine runs, something of the item to read, a question for a person.
    Asked {
        /// How many calls the round asked for.
        asked: usize,
        /// The calls made for it: none until the engine makes them, then all of them.
        made: Vec<Call>,
    },
    /// The blocks the run wrote were handed in.
    HandedIn(Vec<Write>),
}

/// A call made for a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub call: CallId,
    /// Whether the call has its result.
    pub done: bool,
}

/// What the flow requires of what a cell's run returns.
#[derive(Clone, Copy, Debug)]
pub struct Requires<'a> {
    /// The blocks the cell may write.
    pub outputs: &'a [BlockId],
    /// The values of each block that is a choice.
    pub choices: &'a BTreeMap<BlockId, Vec<String>>,
    /// How often what the run returns may be sent back.
    pub returns: u32,
    /// How many calls the run may make.
    pub calls: u32,
}

/// What a run does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// The run has its first round.
    Begin,
    /// A round of the run is under way: nothing is decided until it ends.
    UnderWay,
    /// The run would have its next round, and its work is paused: no round begins until the work is let on.
    Held,
    /// The calls the last round asked for are made.
    Make,
    /// The run rests at these calls, which have no result yet.
    Rest { calls: Vec<CallId> },
    /// Every call the last round asked for has its result: the run has its next round, which is given the results.
    Resume,
    /// What came back does not meet what the flow requires: it is sent back, the next round being given why.
    SendBack { unmet: Vec<Unmet> },
    /// What was handed in meets everything the flow requires: the run finishes, writing these blocks.
    Finish { writes: Vec<Write> },
    /// The run has reached a limit: the owner lets it go on, the count starting anew, or cancels the work.
    AtLimit(Limit),
}

/// A limit a run has reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Limit {
    /// This many returns were sent back, and what the last round returned does not meet what the flow requires
    /// either.
    Returns { sent_back: u32 },
    /// This many calls were made, and with those the last round asks for the run would have made more than it may.
    Calls { made: u32 },
}

/// Why what came back does not meet what the flow requires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unmet {
    /// The round asked for calls, and for none: it neither asks for anything nor hands anything in.
    AsksNothing,
    /// The block is not one the cell may write.
    NotAnOutput { block: BlockId },
    /// The block was written before in the same return.
    WrittenTwice { block: BlockId },
    /// The block is a choice, and was written with no value.
    NoValue { block: BlockId },
    /// The block is a choice, and this is not one of its values.
    NotAValue { block: BlockId, value: String },
    /// The block is no choice, and was written with a value.
    NotAChoice { block: BlockId },
}

/// What the run does next. A pause of its work lets no round begin: where the run would have one, it is held.
pub fn next(log: &Log, requires: Requires, paused: bool) -> Step {
    let Some(last) = log.rounds.last() else {
        return held(Step::Begin, paused);
    };
    let Some(returned) = &last.returned else {
        return Step::UnderWay;
    };
    // The rounds before the last, from where the owner last let the run go on: what has a limit is counted there.
    let before = log.rounds.len() - 1;
    let earlier = &log.rounds[log.let_go.min(before)..before];
    // Whether the owner let the run go on after the last round ended: what that round returned is then taken up
    // whatever the counts were.
    let let_on = log.let_go > before;
    let unmet = match returned {
        Return::Asked { asked: 0, .. } => vec![Unmet::AsksNothing],
        Return::Asked { asked, made } if made.is_empty() => {
            let before: usize = earlier.iter().map(calls).sum();
            if !let_on && before + asked > requires.calls as usize {
                return Step::AtLimit(Limit::Calls { made: before as u32 });
            }
            return Step::Make;
        }
        Return::Asked { made, .. } => {
            let open: Vec<CallId> = made.iter().filter(|c| !c.done).map(|c| c.call).collect();
            if open.is_empty() {
                return held(Step::Resume, paused);
            }
            return Step::Rest { calls: open };
        }
        Return::HandedIn(writes) => {
            let unmet = unmet(writes, requires);
            if unmet.is_empty() {
                return Step::Finish { writes: writes.clone() };
            }
            unmet
        }
    };
    // Every return before this one that handed in, or asked for nothing, was sent back: the run would have finished
    // or made calls otherwise.
    let sent_back = earlier.iter().filter(|r| sent_back(r)).count() as u32;
    if !let_on && sent_back >= requires.returns {
        return Step::AtLimit(Limit::Returns { sent_back });
    }
    held(Step::SendBack { unmet }, paused)
}

/// The step, or its being held where the work is paused: the step begins a round.
fn held(step: Step, paused: bool) -> Step {
    if paused { Step::Held } else { step }
}

/// How many calls were made for the round.
fn calls(round: &Round) -> usize {
    match &round.returned {
        Some(Return::Asked { made, .. }) => made.len(),
        _ => 0,
    }
}

/// Whether what the round returned was sent back: it is not the last round, and it handed in or asked for nothing.
fn sent_back(round: &Round) -> bool {
    matches!(
        round.returned,
        Some(Return::HandedIn(_) | Return::Asked { asked: 0, .. })
    )
}

/// Each way in which what was handed in does not meet what the flow requires, in the order of the writes: one for
/// each write.
fn unmet(writes: &[Write], requires: Requires) -> Vec<Unmet> {
    let mut unmet = Vec::new();
    for (i, write) in writes.iter().enumerate() {
        let block = || write.block.clone();
        if writes[..i].iter().any(|w| w.block == write.block) {
            unmet.push(Unmet::WrittenTwice { block: block() });
            continue;
        }
        if !requires.outputs.contains(&write.block) {
            unmet.push(Unmet::NotAnOutput { block: block() });
            continue;
        }
        match (requires.choices.get(&write.block), &write.value) {
            (Some(_), None) => unmet.push(Unmet::NoValue { block: block() }),
            (Some(values), Some(value)) if !values.contains(value) => unmet.push(Unmet::NotAValue {
                block: block(),
                value: value.clone(),
            }),
            (None, Some(_)) => unmet.push(Unmet::NotAChoice { block: block() }),
            _ => {}
        }
    }
    unmet
}
