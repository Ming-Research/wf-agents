//! Where a round or a call the engine carries out stands as it goes, as what carries it out says: the act of a check
//! waits for the project's turn, which another work holds; a tool or an act runs as a CI run, at its link. No record
//! holds it: it is what the outside says at a moment, kept only in memory while the round or the call is under way,
//! and said again by what carries it out once it is given again after a stop.

use decide::{CallId, Records, RunId};
use store::WorkNumber;

/// Where a round or a call stands as it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Along {
    /// It waits for the project's turn to check and merge, which this work holds: the works of a project check and
    /// merge one at a time.
    Turn { holder: WorkNumber },
    /// It runs as the CI run at this link.
    Runs { at: String },
}

/// What a carrier says where it stands of: a round of a run, by its number in the run, or a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Under {
    Round { run: RunId, round: usize },
    Call(CallId),
}

impl Under {
    /// Whether it is under way by the work's records: the round began and has not ended, or the call was made and has
    /// no result.
    pub fn under_way(&self, records: &Records) -> bool {
        match *self {
            Under::Round { run, round } => {
                let began = records.rounds.iter().filter(|r| r.run == run).count();
                let ended = records.returned.iter().filter(|r| r.run == run).count();
                began >= round && ended < round
            }
            Under::Call(call) => {
                records.called.iter().any(|c| c.call == call) && !records.done.iter().any(|d| d.call == call)
            }
        }
    }
}
