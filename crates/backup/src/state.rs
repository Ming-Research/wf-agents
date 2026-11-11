//! What the records say of the copies, for the owner's page: a function of the records, the time now and the settings.

use serde::Serialize;
use store::{Kept, Records};
use work::Time;

use crate::{PERIOD, TICK};

/// How the copies stand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum State {
    /// The newest copy is recent and uploaded.
    Ok,
    /// Copies are kept on this machine alone, which shares the disk they are kept against losing.
    LocalOnly,
    /// This many copies that are kept were not uploaded yet.
    Pending { n: usize },
    /// The newest copy is older than a day and an hour; `since` is when it was taken, none where there is none.
    Stale { since: Option<Time> },
    /// A copy is recorded as taken after a day from now: the clock went ahead and came back, or it is wrong.
    Clock,
    /// The last step that failed, with nothing done since to mend it.
    Failed { at: Time, step: String, why: String },
}

/// What the page shows of the copies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub state: State,
    /// The time now, by the engine's clock.
    pub now: Time,
    /// When the newest trusted copy was taken.
    pub last_taken: Option<Time>,
    /// When the newest upload was made.
    pub last_uploaded: Option<Time>,
    /// How many copies that are kept were not uploaded yet.
    pub pending: usize,
    /// The failure shown, where the state is `Failed`.
    pub last_failure: Option<Shown>,
}

/// A failure, as the page is told of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Shown {
    pub at: Time,
    pub step: String,
    pub why: String,
}

/// The copies that are kept: the `keep` newest in the order they were recorded in.
pub(crate) fn kept(records: &Records, keep: usize) -> &[Kept] {
    let all = &records.copies;
    &all[all.len().saturating_sub(keep)..]
}

/// Whether a copy was taken in a time the clock can be trusted for, at `now`.
pub(crate) fn trusted(taken: Time, now: Time) -> bool {
    taken.0 <= now.0.saturating_add(PERIOD)
}

/// The failure that is still to be shown: the newest one, if no step ended with nothing failing after it. A step that
/// ends clean has done everything that was to be done, so it mends every failure recorded before it, whatever step it
/// was. What is before and after is told by the order of recording, not by time, which a clock that jumped makes wrong.
fn failure(records: &Records) -> Option<&store::Failure> {
    let newest = records.failures.last()?;
    records
        .mended
        .is_none_or(|mended| newest.seq > mended)
        .then_some(newest)
}

/// How the copies stand at `now`, with the copies kept being the `keep` newest and `configured` being whether a place
/// off the machine is given. The order of what is said: a failure first, then the clock, a copy too old, a copy not
/// uploaded, a place off the machine that is not given, and last that all is well.
pub fn report(records: &Records, now: Time, configured: bool, keep: usize) -> Report {
    let kept = kept(records, keep);
    let pending = if configured {
        kept.iter().filter(|k| !records.uploads.contains_key(&k.name)).count()
    } else {
        0
    };
    let trusted_taken = kept.iter().filter(|k| trusted(k.taken, now)).map(|k| k.taken).max();
    let last_uploaded = records.uploads.values().copied().max();
    let shown = failure(records);
    let state = if let Some(f) = shown {
        State::Failed {
            at: f.at,
            step: f.step.clone(),
            why: f.why.clone(),
        }
    } else if kept.iter().any(|k| !trusted(k.taken, now)) {
        State::Clock
    } else if trusted_taken.is_none_or(|t| t.0.saturating_add(PERIOD + TICK) < now.0) {
        State::Stale { since: trusted_taken }
    } else if pending > 0 {
        State::Pending { n: pending }
    } else if !configured {
        State::LocalOnly
    } else {
        State::Ok
    };
    Report {
        state,
        now,
        last_taken: trusted_taken,
        last_uploaded,
        pending,
        last_failure: shown.map(|f| Shown {
            at: f.at,
            step: f.step.clone(),
            why: f.why.clone(),
        }),
    }
}
