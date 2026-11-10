//! What a run does next, from what it has done so far.
//!
//! A run is made of rounds. At the end of a round its agent, or the engine's own act, either asks for calls, whose
//! results the next round is given, or hands in the blocks it wrote. A return that does not meet what the flow
//! requires of it is sent back: the next round is given why. A run finishes when a return meets everything, and
//! writes its blocks then. How often a return is sent back and how many calls are made each has a limit, at which the
//! owner lets the run go on or cancels the work.
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
    Asked(Vec<Call>),
    /// The blocks the run wrote were handed in.
    HandedIn(Vec<Write>),
}

/// A call a round asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub call: CallId,
    /// Whether the call has its result.
    pub done: bool,
}

/// What the flow requires of what a cell hands in.
#[derive(Clone, Copy, Debug)]
pub struct Requires<'a> {
    /// The blocks the cell may write.
    pub outputs: &'a [BlockId],
    /// The values of each block that is a choice.
    pub choices: &'a BTreeMap<BlockId, Vec<String>>,
    /// How often a return of the run may be sent back.
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
    /// The run rests at these calls, which have no result yet.
    Rest { calls: Vec<CallId> },
    /// Every call the last round asked for has its result: the run has its next round, which is given the results.
    Resume,
    /// What was handed in does not meet what the flow requires: it is sent back, the next round being given why.
    SendBack { unmet: Vec<Unmet> },
    /// What was handed in meets everything the flow requires: the run finishes, writing these blocks.
    Finish { writes: Vec<Write> },
    /// The run has reached a limit: the owner lets it go on, the count starting anew, or cancels the work.
    AtLimit(Limit),
}

/// A limit a run has reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Limit {
    /// This many returns were sent back, and the last one handed in does not meet what the flow requires either.
    Returns { sent_back: u32 },
    /// This many calls were made, and the last round asks for more.
    Calls { made: u32 },
}

/// Why what was handed in does not meet what the flow requires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unmet {
    /// The block is not one the cell may write.
    NotAnOutput { block: BlockId },
    /// The block was written twice in one return.
    WrittenTwice { block: BlockId },
    /// The block is a choice, and was written with no value.
    NoValue { block: BlockId },
    /// The block is a choice, and this is not one of its values.
    NotAValue { block: BlockId, value: String },
    /// The block is no choice, and was written with a value.
    NotAChoice { block: BlockId },
}

/// What the run does next.
pub fn next(log: &Log, requires: Requires) -> Step {
    let Some(last) = log.rounds.last() else {
        return Step::Begin;
    };
    let Some(returned) = &last.returned else {
        return Step::UnderWay;
    };
    // The rounds before the last, from where the owner last let the run go on: what has a limit is counted there.
    let before = log.rounds.len() - 1;
    let earlier = &log.rounds[log.let_go.min(before)..before];
    match returned {
        Return::Asked(calls) => {
            let open: Vec<CallId> = calls.iter().filter(|c| !c.done).map(|c| c.call).collect();
            if !open.is_empty() {
                return Step::Rest { calls: open };
            }
            Step::Resume
        }
        Return::HandedIn(writes) => {
            let unmet = unmet(writes, requires);
            if unmet.is_empty() {
                return Step::Finish { writes: writes.clone() };
            }
            // Every return handed in before this one was sent back: the run would have finished otherwise.
            let handed_in = (earlier.iter()).filter(|r| matches!(r.returned, Some(Return::HandedIn(_))));
            let sent_back = handed_in.count() as u32;
            if sent_back >= requires.returns {
                return Step::AtLimit(Limit::Returns { sent_back });
            }
            Step::SendBack { unmet }
        }
    }
}

/// Whether the calls a round asks for may be made: not when, with them, the run would have made more than its limit
/// since the owner last let it go on.
pub fn may_call(log: &Log, asked: usize, requires: Requires) -> Result<(), Limit> {
    let counted = log.rounds.get(log.let_go..).unwrap_or_default();
    let made: usize = (counted.iter())
        .map(|r| match &r.returned {
            Some(Return::Asked(calls)) => calls.len(),
            _ => 0,
        })
        .sum();
    if made + asked > requires.calls as usize {
        return Err(Limit::Calls { made: made as u32 });
    }
    Ok(())
}

/// Each way in which what was handed in does not meet what the flow requires, in the order of the writes.
fn unmet(writes: &[Write], requires: Requires) -> Vec<Unmet> {
    let mut unmet = Vec::new();
    for (i, write) in writes.iter().enumerate() {
        let block = || write.block.clone();
        if !requires.outputs.contains(&write.block) {
            unmet.push(Unmet::NotAnOutput { block: block() });
            continue;
        }
        if writes[..i].iter().any(|w| w.block == write.block) {
            unmet.push(Unmet::WrittenTwice { block: block() });
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
