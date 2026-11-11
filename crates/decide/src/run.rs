//! What a run does next, from what it has done so far.
//!
//! A run is made of rounds. At the end of a round its agent, or the engine's own act, either asks for calls, which
//! the engine then makes and whose results the next round is given, or hands in the blocks it wrote. What came back
//! and does not meet what the flow requires is sent back: the next round is given why. A run finishes when what was
//! handed in meets everything, and writes its blocks then. How often something is sent back and how many calls are
//! made each has a limit, at which the owner lets the run go on or cancels the work.
//!
//! A round may also fail: nothing came back, as when whatever carries it out could not start or gave nothing usable.
//! Whatever carried it out tells the cause apart ([`Failure`]), and each kind is handled its own way. A failure in
//! passing is tried again, after a wait that grows each time, as a call of the run that the engine makes; so many in a
//! row and the next pauses the work ([`pausing`]). A return that could not be read is sent back, as any return that does
//! not meet what the flow requires. A round the service's safety check flagged is tried again, in a new session of its
//! line, and the second in a row pauses the work. A cause every work shares (the account's usage limit, a command line
//! that cannot run, no network) holds the agents' rounds of every work, which is no matter of one run's: the run only
//! has its next round once they may begin again. Any other failure pauses the work as the round fails, so that the
//! owner sees the cause first. Once the work is let on, the run has its next round, given what the failed round was
//! given, and the failures counted in a row start anew. A failure is no return sent back, unless it is one that could
//! not be read, and makes no call.
//!
//! A pause of the work holds back the rounds of a run, unless the run started for a note a person wrote: such a run has
//! every round it needs while the work is paused, but the round after one that failed and paused the work, which still
//! waits for the work to be let on.
//!
//! The walk says which cell runs and which run goes on; this says what going on is.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::flow::BlockId;
use crate::records::{CallId, Write};

/// What a run has done so far: its rounds, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    pub rounds: Vec<Round>,
    /// How many of the rounds had ended when the owner last let the run go on at a limit; none at first. The counts
    /// that have a limit start anew from there.
    pub let_go: usize,
    /// How many of the rounds had ended when the owner last let the work on after a pause; none at first. The failures
    /// counted in a row start anew from there.
    pub let_on: usize,
}

/// How many failures in passing in a row are tried again: the next one pauses the work, running again not having
/// helped.
pub const PASSING: u32 = 8;

/// How long the engine waits after the first failure in passing in a row before the round is tried again, in
/// milliseconds; each later one in the row waits twice as long as the one before, up to [`LONGEST_WAIT`].
pub const FIRST_WAIT: u64 = 10_000;

/// The longest the engine waits before a round that failed in passing is tried again, in milliseconds.
pub const LONGEST_WAIT: u64 = 300_000;

/// How long the engine waits after the round that failed in passing, the `failed`th in a row, before it is tried
/// again, in milliseconds.
pub fn wait(failed: u32) -> u64 {
    let doublings = failed.saturating_sub(1).min(16);
    FIRST_WAIT.saturating_mul(1 << doublings).min(LONGEST_WAIT)
}

/// Why a round failed, as whatever carried it out told it apart. Each kind is handled its own way (the module's
/// documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// In passing: the service was overloaded, gave an error or took too long, or the command line crashed. The round
    /// is tried again after a wait.
    Passing,
    /// What the agent returned could not be read: the command line gave up on it, or it is not of the form of a
    /// return. It is sent back.
    Unread,
    /// The service's safety check flagged the round, stopping it or refusing its message. It is tried again in a new
    /// session of its line, which does not give the check the same context again.
    Flagged,
    /// The account's usage limit was reached: no agent round of any work begins until it resets.
    UsageLimit,
    /// The agents' command line cannot run at all: its sign-in has expired, or the program is missing. Only the owner
    /// can mend it: no agent round of any work begins, but one now and then that tries.
    CannotRun,
    /// The network cannot be reached: no agent round of any work begins until it can be.
    Offline,
    /// The prompt is too long for the model: running it again cannot help, and the work is paused.
    PromptTooLong,
    /// A cause no kind tells apart that is no report of the agents' command line: the engine's own act failed, or
    /// the round could not be carried out at all. The work is paused. Rounds that failed before kinds were told apart
    /// read so, since each paused its work.
    #[default]
    Other,
}

impl Failure {
    /// Whether every work shares the cause, so that it holds the agents' rounds of every work rather than one run.
    pub fn shared(self) -> bool {
        matches!(self, Failure::UsageLimit | Failure::CannotRun | Failure::Offline)
    }
}

/// Why the engine pauses the work as a round of its run fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pausing {
    /// Running the round again cannot help, by its kind.
    Failed(Failure),
    /// The round failed in passing this many times in a row, and running it again has not helped.
    Passing { failed: u32 },
    /// The round was flagged once more, in the new session its line was given for the time before.
    FlaggedAgain,
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
    /// The round failed: nothing came back, for this kind of cause, said so.
    Failed {
        failure: Failure,
        why: String,
        /// The waits the engine made as calls after it, before it is tried again: none until the engine makes one.
        waits: Vec<Call>,
    },
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
    /// The blocks among those that every hand-in of the cell includes.
    pub must_write: &'a [BlockId],
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
    /// The run would have its next round, and the pause of its work holds that round back until the work is let on.
    Held,
    /// The calls the last round asked for are made.
    Make,
    /// The run rests at these calls, which have no result yet.
    Rest { calls: Vec<CallId> },
    /// Every call the last round asked for has its result: the run has its next round, which is given the results.
    Resume,
    /// The last round failed in passing, the `failed`th time in a row: the engine waits this long from its end, in
    /// milliseconds, as a call of the run that it makes itself, before the round is tried again.
    Wait { failed: u32, ms: u64 },
    /// The last round failed: the run has its next round, which is given what the failed round was given.
    Retry,
    /// What came back does not meet what the flow requires: it is sent back, the next round being given why.
    SendBack { unmet: Vec<Unmet> },
    /// What was handed in meets everything the flow requires: the run finishes, writing these blocks.
    Finish { writes: Vec<Write> },
    /// The run has reached a limit: the owner lets it go on, the count starting anew, or cancels the work.
    AtLimit(Limit),
}

/// A limit a run has reached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Limit {
    /// This many returns were sent back, and what the last round returned does not meet what the flow requires
    /// either.
    Returns { sent_back: u32 },
    /// This many calls were made, and with those the last round asks for the run would have made more than it may.
    Calls { made: u32 },
}

/// Why what came back does not meet what the flow requires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The block is one the cell must hand in each time, and was not handed in.
    NotHandedIn { block: BlockId },
    /// What came back could not be read as a return, for this reason, as whatever carried the round out says.
    Unreadable { why: String },
}

/// What a pause of the run's work holds back of its rounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// Nothing: the work is not paused, or it is and the run started for a note a person wrote, and no failure of it
    /// paused the work since its last round ended.
    Nothing,
    /// Only the round after one that failed: the work is paused, the run started for a note a person wrote, and its
    /// last round's failure paused the work.
    Retry,
    /// Every round: the work is paused, and the run did not start for a note a person wrote.
    Rounds,
}

/// What the run does next. Where the run would have a round that the pause of its work holds back, it is held.
pub fn next(log: &Log, requires: Requires, hold: Hold) -> Step {
    let Some(last) = log.rounds.last() else {
        return held(Step::Begin, hold);
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
        // What could not be read is sent back, as a return that is not of its form.
        Return::Failed {
            failure: Failure::Unread,
            why,
            ..
        } => vec![Unmet::Unreadable { why: why.clone() }],
        // In passing, the engine waits before the round is tried again; one let on since, or that paused the work,
        // waits no more.
        Return::Failed {
            failure: Failure::Passing,
            waits,
            ..
        } => {
            let failed = passing_in_a_row(log);
            if failed == 0 || failed > PASSING {
                return held(Step::Retry, hold);
            }
            if waits.is_empty() {
                return Step::Wait {
                    failed,
                    ms: wait(failed),
                };
            }
            let open: Vec<CallId> = waits.iter().filter(|c| !c.done).map(|c| c.call).collect();
            if open.is_empty() {
                return held(Step::Retry, hold);
            }
            return Step::Rest { calls: open };
        }
        // Any other failure is no return sent back and makes no call: it counts against no limit. A flagged round is
        // tried in a new session of its line, which whatever carries it out begins; one whose cause every work
        // shares has its next round once agents' rounds may begin again; one that paused the work, once it is let on.
        Return::Failed { .. } => return held(Step::Retry, hold),
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
                return held(Step::Resume, hold);
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
    held(Step::SendBack { unmet }, hold)
}

/// The step, which begins a round, or its being held where the pause holds that round back.
fn held(step: Step, hold: Hold) -> Step {
    let holds = match hold {
        Hold::Nothing => false,
        Hold::Retry => step == Step::Retry,
        Hold::Rounds => true,
    };
    if holds { Step::Held } else { step }
}

/// How many calls were made for the round.
fn calls(round: &Round) -> usize {
    match &round.returned {
        Some(Return::Asked { made, .. }) => made.len(),
        _ => 0,
    }
}

/// Whether what the round returned was sent back: it is not the last round, and it handed in, asked for nothing or
/// returned what could not be read. A round that failed otherwise returned nothing to send back.
fn sent_back(round: &Round) -> bool {
    matches!(
        round.returned,
        Some(
            Return::HandedIn(_)
                | Return::Asked { asked: 0, .. }
                | Return::Failed {
                    failure: Failure::Unread,
                    ..
                }
        )
    )
}

/// The failures the run's last rounds ended in, in a row, the last first: those since the owner last let the work on,
/// up to the last round that did not fail.
fn failures_in_a_row(log: &Log) -> impl Iterator<Item = Failure> + '_ {
    let since = log.let_on.min(log.rounds.len());
    (log.rounds[since..].iter().rev()).map_while(|round| match &round.returned {
        Some(Return::Failed { failure, .. }) => Some(*failure),
        _ => None,
    })
}

/// How many of the run's last rounds in a row failed in passing ([`failures_in_a_row`]).
fn passing_in_a_row(log: &Log) -> u32 {
    failures_in_a_row(log).filter(|f| *f == Failure::Passing).count() as u32
}

/// Why the engine pauses the work as the run's last round fails, if it does: the failure is of a kind running again
/// cannot help; or it is in passing, and more of the run's last rounds in a row failed so than are tried again
/// ([`PASSING`]); or the round was flagged, and so was another of the run's last rounds in a row, whatever failed between
/// them, the first having had its line's session begun anew. Any other failure the run takes up itself ([`next`]).
/// The failures in a row start anew where the owner let the work on.
pub fn pausing(log: &Log) -> Option<Pausing> {
    let mut row = failures_in_a_row(log);
    match row.next()? {
        failure @ (Failure::PromptTooLong | Failure::Other) => Some(Pausing::Failed(failure)),
        Failure::Passing => {
            let failed = passing_in_a_row(log);
            (failed > PASSING).then_some(Pausing::Passing { failed })
        }
        Failure::Flagged => row.any(|f| f == Failure::Flagged).then_some(Pausing::FlaggedAgain),
        Failure::Unread | Failure::UsageLimit | Failure::CannotRun | Failure::Offline => None,
    }
}

/// Each way in which what was handed in does not meet what the flow requires: one for each write, in the order of the
/// writes, then one for each block the cell must hand in and did not, in the order the cell lists them.
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
    for block in requires.must_write {
        if !writes.iter().any(|w| w.block == *block) {
            unmet.push(Unmet::NotHandedIn { block: block.clone() });
        }
    }
    unmet
}
