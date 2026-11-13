//! What a run does next, from what it has done so far.
//!
//! A run is one entering of its cell (design document `data-and-context`, 7). It is made of rounds: the first, each one
//! after a return was sent back, and each one after a round failed. At the end of a round its agent, or the engine's
//! own act, returns, and a return is of one of two kinds (design document `data-and-context`, 30): it asks for calls
//! and for nothing else, dealing with nothing; or it does the cell's work, saying which of what ran the round it has
//! dealt with and writing its blocks. A return that does not meet what the flow requires, one that mixes the two among
//! it, is sent back: the next round is given why. One that meets it is taken, and ends the run: the calls it asks for
//! are made, or the run writes its blocks and what it dealt with is consumed. A cell whose run asked for calls is
//! entered again as a new run once their results are in, which the walk says. How often a return is sent back has a
//! limit, at which the owner lets the run go on or cancels the work.
//!
//! A round may also fail: nothing came back, as when whatever carries it out could not start or gave nothing usable.
//! Whatever carried it out tells the cause apart ([`Failure`]), and each kind is handled its own way. A failure in
//! passing is tried again, after a wait that grows each time, as a call of the run that the engine makes; so many in a
//! row and the next pauses the work ([`pausing`]). A return that could not be read is sent back, as any return that
//! does not meet what the flow requires. A round the service's safety check flagged is tried again, in a new session of
//! its line, and the second in a row pauses the work. A cause every work shares (the account's usage limit, a command
//! line that cannot run, no network) holds the agents' rounds of every work, which is no matter of one run's: the run
//! only has its next round once they may begin again. Any other failure pauses the work as the round fails, so that the
//! owner sees the cause first. Once the work is let on, the run has its next round, given what the failed round was
//! given, and the failures counted in a row start anew. A round that failed, or a return that was sent back, deals with
//! nothing.
//!
//! A pause of the work holds back the rounds of a run, unless the run started for a note a person wrote: such a run has
//! every round it needs while the work is paused, but the round after one that failed and paused the work, which still
//! waits for the work to be let on.
//!
//! The walk says which cell runs and which run goes on; this says what going on is.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::flow::BlockId;
use crate::records::{CallId, Seq, Write};

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
    /// The return was read: what of what ran the round it dealt with, in the order it named them, the blocks it writes,
    /// and how many calls it asks for.
    Came {
        dealt_with: Vec<BlockId>,
        writes: Vec<Write>,
        calls: usize,
    },
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

/// What the run's round was told, and how the blocks its cell may write stand now, as the walk reads them from the
/// records ([`crate::told`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Told {
    /// What ran the round, which its return may name as dealt with: each block new to the cell as the run started, in
    /// the order the cell names them, then each block put in to the round since that was not among them.
    pub ran_by: Vec<BlockId>,
    /// Of each block the cell may write, the version the round was told of and the latest.
    pub outputs: BTreeMap<BlockId, Standing>,
    /// Whether the round's cell still waits for the results of calls its runs asked for: the round was entered for the
    /// owner turning to the cell meanwhile, answers and goes on waiting, and so is held to no block its cell must write
    /// (design document `data-and-context`, 40, 42).
    pub waits_for_calls: bool,
}

/// A block a cell may write: the version its run's round was told of, and the latest, with its value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Standing {
    /// The latest version the round was told of: the one the run started from, or one put in to it since.
    pub told: Option<Seq>,
    /// The block's latest version.
    pub latest: Option<Seq>,
    /// The latest version's value, where the block is a choice.
    pub value: Option<String>,
}

/// What the flow requires of what a cell's run returns.
#[derive(Clone, Copy, Debug)]
pub struct Requires<'a> {
    /// The blocks the cell may write.
    pub outputs: &'a [BlockId],
    /// The blocks among those that a return dealing with everything that ran its round, and asking for no call,
    /// includes (plan, D8), unless its cell still waits for the results of calls ([`Told::waits_for_calls`]).
    pub must_write: &'a [BlockId],
    /// The values of each block that is a choice.
    pub choices: &'a BTreeMap<BlockId, Vec<String>>,
    /// How often what the run returns may be sent back.
    pub returns: u32,
    /// What the round was told, and how the blocks it may write stand.
    pub told: &'a Told,
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
    /// The run rests at these calls, which have no result yet.
    Rest { calls: Vec<CallId> },
    /// The last round failed in passing, the `failed`th time in a row: the engine waits this long from its end, in
    /// milliseconds, as a call of the run that it makes itself, before the round is tried again.
    Wait { failed: u32, ms: u64 },
    /// The last round failed: the run has its next round, which is given what the failed round was given.
    Retry,
    /// What came back does not meet what the flow requires: it is sent back, the next round being given why.
    SendBack { unmet: Vec<Unmet> },
    /// What came back meets everything the flow requires: the calls it asks for are made, and the run finishes,
    /// having dealt with these and writing these blocks.
    Finish {
        dealt_with: Vec<BlockId>,
        writes: Vec<Write>,
    },
    /// The run has reached a limit: the owner lets it go on, the count starting anew, or cancels the work.
    AtLimit(Limit),
}

/// A limit a run has reached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Limit {
    /// This many returns were sent back, and what the last round returned does not meet what the flow requires
    /// either.
    Returns { sent_back: u32 },
    /// This many calls were made, and with those the last round asks for the run would have made more than it may. A
    /// run reached it before a return that asks ended its run; the walk now bounds how often a cell is entered again
    /// for the results of what it asked for.
    Calls { made: u32 },
}

/// Why what came back does not meet what the flow requires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unmet {
    /// The round asked for calls, and for none: it neither asked for anything nor handed anything in. Returns of the
    /// form before a return said what it dealt with were sent back so.
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
    /// The return names the block as dealt with, and it is not among what ran the round.
    NotRunBy { block: BlockId },
    /// The return names the block as dealt with twice.
    DealtWithTwice { block: BlockId },
    /// Something ran the round, and the return deals with none of it and asks for no call: the cell would be entered
    /// again at once for the same.
    DealtWithNothing,
    /// The block got this version from another writer after the round was told of the one it writes over: the return
    /// would write over what the round never saw. The next round is told of the version.
    WrittenOver { block: BlockId, version: Seq },
    /// The block is a choice, written as changing nothing in meaning with another value than the one it has: a choice
    /// changes its meaning when its value does (decision `meaning-told-by-the-writer`).
    ChoiceChanged { block: BlockId },
    /// The return asks for calls and also does the cell's work, dealing with something or writing a block: a return
    /// that asks for calls asks for nothing else, and deals with nothing (design document `data-and-context`, 30).
    AsksAndWorks,
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
        Return::Came {
            dealt_with,
            writes,
            calls,
        } => {
            let unmet = unmet(dealt_with, writes, *calls, requires);
            if unmet.is_empty() {
                return Step::Finish {
                    dealt_with: dealt_with.clone(),
                    writes: writes.clone(),
                };
            }
            unmet
        }
    };
    // Every return before this one that was read, or could not be, was sent back: the run would have finished
    // otherwise.
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

/// Whether what the round returned was sent back: it is not the last round, and it was read or could not be. A round
/// that failed otherwise returned nothing to send back.
fn sent_back(round: &Round) -> bool {
    matches!(
        round.returned,
        Some(
            Return::Came { .. }
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
/// ([`PASSING`]); or the round was flagged, and so was another of the run's last rounds in a row, whatever failed
/// between them, the first having had its line's session begun anew. Any other failure the run takes up itself
/// ([`next`]). The failures in a row start anew where the owner let the work on.
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

/// Each way in which what came back does not meet what the flow requires, in one pass (principle `report-all-at-once`):
/// first what it says it dealt with, in its order; then one for each write, in the order of the writes; then whether
/// it asks for calls beside doing the cell's work; then whether it deals with nothing while something ran its round
/// and asks for no call; then each block the cell must hand in
/// and did not, in the order the cell lists them, where the return deals with everything that ran its round and asks
/// for no call.
fn unmet(dealt_with: &[BlockId], writes: &[Write], calls: usize, requires: Requires) -> Vec<Unmet> {
    let told = requires.told;
    let mut unmet = Vec::new();
    for (i, block) in dealt_with.iter().enumerate() {
        if dealt_with[..i].contains(block) {
            unmet.push(Unmet::DealtWithTwice { block: block.clone() });
        } else if !told.ran_by.contains(block) {
            unmet.push(Unmet::NotRunBy { block: block.clone() });
        }
    }
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
        let standing = told.outputs.get(&write.block);
        if let Some(Standing {
            told,
            latest: Some(latest),
            ..
        }) = standing
            && *told != Some(*latest)
        {
            unmet.push(Unmet::WrittenOver {
                block: block(),
                version: *latest,
            });
        }
        let choice = requires.choices.contains_key(&write.block);
        let value = standing.and_then(|s| s.value.as_ref());
        if choice && write.meaning_unchanged && value != write.value.as_ref() {
            unmet.push(Unmet::ChoiceChanged { block: block() });
        }
    }
    if calls > 0 && !(dealt_with.is_empty() && writes.is_empty()) {
        unmet.push(Unmet::AsksAndWorks);
    }
    let everything = told.ran_by.iter().all(|b| dealt_with.contains(b));
    if calls == 0 && dealt_with.is_empty() && !told.ran_by.is_empty() {
        unmet.push(Unmet::DealtWithNothing);
    }
    if calls == 0 && everything && !told.waits_for_calls {
        for block in requires.must_write {
            if !writes.iter().any(|w| w.block == *block) {
                unmet.push(Unmet::NotHandedIn { block: block.clone() });
            }
        }
    }
    unmet
}
