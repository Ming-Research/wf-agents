//! The judge of a play (design document `testing`, 14): it reads a work's records against the path its script states
//! (module `script::items`), by the reading of the records the histories and the exploration judge by (crate
//! `reading`) and never by the deciding code the play tests, and names the first step that left the path, with what
//! the records hold there.
//!
//! The records hold, in the order of their places, each step of a path but a few waits: a run of a cell started; a
//! round that failed, with its kind; a return sent back, which the run's next round is given; a call a run rests at,
//! for a tool, a person, the engine's wait before a round is tried again, or the owner's word on the engine's own call
//! at a run's limit; a question that waits for the owner's answer; what a run wrote waiting for a source outside the
//! work; the work paused and let on, by whom; a cell let go at its limit; a cancel, a reopen, and the merge's finished
//! run. A wait on a precondition, or at a cell's limit of a sender, no entry records: it is read where the work came to
//! rest after a thing that happened to it, with nothing of it under way and no call, question or source waited for,
//! at the cell the reading finds it stands at, where a key of that cell's preconditions does not hold that a person or
//! a source outside the work can make hold (one only cells make hold, none of which runs, is a stall), or where the
//! cell would run once more for what a sender wrote that sent it back as often as the flow's `again` lets one since its
//! counts last began anew (decision the-same-sender-again); the wait that lasts over several things that happened is
//! one step. The pause the work was added with, and the letting on a play gives it as it adds it, are the item's start
//! and no step of its path.
//!
//! What no record holds is read from the engine's view of the work now, and so only as a path's end: a stall, which
//! the reading cross-checks (no cell that applies has anything to be entered for, no call is owed); a wait for a slot,
//! for the turn to check and merge, and for what every work shares, which are kept in memory only (decisions
//! slots-in-the-scheduler, one-work-checks-and-merges-at-a-time and the-engine-checks-the-outside-itself); and the
//! pause of everything, which the installation keeps. A round under way, which the records hold, is a path's end too.

use std::fmt;

use decide::{CellId, Decision, Pauser};
use driver::{Along, Driver};
use flow::{Compiled, Kind};
use store::WorkNumber;
use work::Work;

use crate::script::items::Step;

/// What the engine's view of a work shows now, for what no record of it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    /// What the work does next, in the room the engine leaves it.
    pub decision: Decision,
    /// Whether a round of it under way says it waits for the turn to check and merge.
    pub turn: bool,
    /// Whether the wait every work shares holds the round it would begin next.
    pub shared: bool,
    /// The pause of everything, by who set it, while it holds.
    pub everything: Option<Pauser>,
}

impl Shown {
    /// What the driver shows of the work now; none where it has no such work.
    pub fn of(driver: &Driver, number: WorkNumber) -> Option<Shown> {
        let kept = driver.work(number)?;
        let decision = driver.view(number)?.decision;
        let turn = (driver.along_of(number).iter()).any(|(_, along)| matches!(along, Along::Turn { .. }));
        let gate = driver.gate_of(number);
        let kind = |cell: &CellId| kept.flow.cells.iter().find(|c| c.id == *cell).map(|c| c.kind);
        let shared = match &decision {
            Decision::Run { cell, .. } | Decision::GoOn { cell, .. } => match kind(cell) {
                Some(Kind::Agent) => gate.holds_agents(),
                Some(Kind::Engine) => gate.holds_acts(),
                None => false,
            },
            _ => false,
        };
        let everything = driver.everything_paused().map(|paused| paused.by);
        Some(Shown {
            decision,
            turn,
            shared,
            everything,
        })
    }
}

/// How a work's records stand against its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Judged {
    /// The work took its path and stands at its end.
    Took,
    /// The work took its path up to the step of this place, counted from one, which it has not taken, or, its last,
    /// no longer stands at: the records hold nothing past the steps before it, and what the engine shows of the work now
    /// is this, where it is said. More may yet come.
    Short {
        step: usize,
        said: String,
        shown: Option<String>,
    },
    /// The step of this place in the path, counted from one, left it: the path says this there, or, past its end,
    /// nothing, and the records hold this.
    Left {
        step: usize,
        said: Option<String>,
        records: String,
    },
}

impl fmt::Display for Judged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Judged::Took => f.write_str("took its path"),
            Judged::Short {
                step,
                said,
                shown: None,
            } => write!(
                f,
                "did not take step {step} of its path, {said}: the records hold nothing more"
            ),
            Judged::Short {
                step,
                said,
                shown: Some(shown),
            } => write!(
                f,
                "does not stand at step {step} of its path, {said}: the engine shows {shown}"
            ),
            Judged::Left {
                step,
                said: Some(said),
                records,
            } => write!(f, "left its path at step {step}, {said}: the records hold {records}"),
            Judged::Left {
                said: None, records, ..
            } => write!(f, "went on past the end of its path: the records hold {records}"),
        }
    }
}

/// A step the records hold, with whether the work still stands at it as they end: a wait that has not ended, a pause
/// not lifted, a cancel that stands; any other step stands once taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub step: Step,
    pub stands: bool,
}

/// How the work's records stand against its path: nothing of them is judged yet.
pub fn judge(flow: &Compiled, work: &Work, held: bool, path: &[Step], shown: &Shown) -> Judged {
    let _ = (flow, work, held, path, shown);
    Judged::Took
}

/// The states the engine shows the work at now, as steps: none are read yet.
pub fn states(work: &Work, shown: &Shown) -> Vec<Step> {
    let _ = (work, shown);
    Vec::new()
}

/// The steps the work's records hold: none are read yet.
pub fn seen(flow: &Compiled, work: &Work, held: bool) -> Vec<Seen> {
    let _ = (flow, work, held);
    Vec::new()
}
